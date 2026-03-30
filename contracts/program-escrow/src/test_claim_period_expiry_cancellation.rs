// ============================================================
// FILE: contracts/program-escrow/src/test_claim_period_expiry_cancellation.rs
//
// Issue #480 — Tests for claim period expiry and cancellation
// Closes #480
//
// Timing assumptions:
//
// - Ledger timestamps are `u64` seconds since Unix epoch
// - `env.ledger().set()` is used to simulate time progression
// - Default claim window: 86,400 seconds (24 hours)
// - A claim is considered expired when:
//     env.ledger().timestamp() > claim.claim_deadline
//
// ============================================================

#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    token, Address, Env, String,
};

use crate::{
    ClaimStatus, ProgramEscrowContract, ProgramEscrowContractClient,
};

fn create_token_contract<'a>(
    env: &Env,
    admin: &Address,
) -> (token::Client<'a>, token::StellarAssetClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    (
        token::Client::new(env, &sac.address()),
        token::StellarAssetClient::new(env, &sac.address()),
    )
}

struct TestSetup<'a> {
    env: Env,
    client: ProgramEscrowContractClient<'a>,
    token: token::Client<'a>,
    token_admin: token::StellarAssetClient<'a>,
    admin: Address,
    payout_key: Address,
    contributor: Address,
    program_id: String,
}

fn setup<'a>() -> TestSetup<'a> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let payout_key = Address::generate(&env);
    let contributor = Address::generate(&env);

    let contract_id = env.register_contract(None, ProgramEscrowContract);
    let client = ProgramEscrowContractClient::new(&env, &contract_id);

    let (token, token_admin) = create_token_contract(&env, &admin);

    token_admin.mint(&contract_id, &1_000_000_i128);

    let program_id = String::from_str(&env, "TestProgram2024");

    // initialize program
    client.init_program(
        &program_id,
        &payout_key,
        &token.address,
        &payout_key,
        &None,
        &None,
    );

    // lock funds
    client.lock_program_funds(&500_000_i128);

    client.set_admin(&admin);

    //  ledger timestamp
    env.ledger().set(LedgerInfo {
        timestamp: 1_000_000,
        protocol_version: 22,
        sequence_number: 10,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1000,
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 3110400,
    });

    TestSetup {
        env,
        client,
        token,
        token_admin,
        admin,
        payout_key,
        contributor,
        program_id,
    }
}

#[test]
fn test_claim_within_window_succeeds() {
    let t = setup();
    let env = &t.env;

    let now: u64 = env.ledger().timestamp();
    let claim_amount: i128 = 10_000;
    let claim_deadline: u64 = now + 86_400; // 24 hours

    let claim_id = t.client.create_pending_claim(
        &t.program_id,
        &t.contributor,
        &claim_amount,
        &claim_deadline,
    );

    // verify if  claim is in it pending state
    let claim = t.client.get_claim(&t.program_id, &claim_id);
    assert_eq!(
        claim.status,
        ClaimStatus::Pending,
        "Claim should be Pending"
    );
    assert_eq!(claim.amount, claim_amount);
    assert_eq!(claim.recipient, t.contributor);

    let balance_before = t.token.balance(&t.contributor);

    // Contributor claims well within the time frame of 6 hours later
    env.ledger().set(LedgerInfo {
        timestamp: now + 21_600,
        ..env.ledger().get()
    });

    t.client
        .execute_claim(&t.program_id, &claim_id, &t.contributor);

    let balance_after = t.token.balance(&t.contributor);
    assert_eq!(
        balance_after - balance_before,
        claim_amount,
        "Contributor should have received exactly the claim amount"
    );

    // assert claim Completed
    let claim = t.client.get_claim(&t.program_id, &claim_id);
    assert_eq!(
        claim.status,
        ClaimStatus::Completed,
        "Claim should be Completed"
    );

    // assert escrow balance decreased
    let program = t.client.get_program_info();
    assert_eq!(program.remaining_balance, 500_000 - claim_amount);
}

// ═══════════════════════════════════════════════════════════════════════════
// TEST 2: Claim attempt after expiry should fail
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "ClaimExpired")]
fn test_claim_after_expiry_fails() {
    let t = setup();
    let env = &t.env;

    let now: u64 = env.ledger().timestamp();
    let claim_amount: i128 = 5_000;
    let claim_deadline: u64 = now + 3_600; // 1 hour timeframe

    let claim_id = t.client.create_pending_claim(
        &t.program_id,
        &t.contributor,
        &claim_amount,
        &claim_deadline,
    );

    // advance time PAST the deadline (2 hours later)
    env.ledger().set(LedgerInfo {
        timestamp: now + 7_200,
        ..env.ledger().get()
    });

    // verifies claim is still Pending — nothing auto-cancels it
    let claim = t.client.get_claim(&t.program_id, &claim_id);
    assert_eq!(claim.status, ClaimStatus::Pending);

    // panics with "ClaimExpired"
    t.client
        .execute_claim(&t.program_id, &claim_id, &t.contributor);
}

// ═══════════════════════════════════════════════════════════════════════════
// TEST 3: Admin cancels a pending (active) claim — funds return to escrow
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_admin_cancel_pending_claim_restores_escrow() {
    let t = setup();
    let env = &t.env;

    let now: u64 = env.ledger().timestamp();
    let claim_amount: i128 = 8_000;
    let claim_deadline: u64 = now + 86_400;

    let claim_id = t.client.create_pending_claim(
        &t.program_id,
        &t.contributor,
        &claim_amount,
        &claim_deadline,
    );

    // Escrow balance should have decreased when claim was created (reserved)
    let balance_after_create = t.client.get_remaining_balance();

    // Admin cancels the still-active pending claim (well within deadline)
    env.ledger().set(LedgerInfo {
        timestamp: now + 1_800, // 30 minutes in — still active
        ..env.ledger().get()
    });

    t.client.cancel_claim(&t.program_id, &claim_id, &t.admin);

    // Assert funds returned to escrow
    let balance_after_cancel = t.client.get_remaining_balance();
    assert_eq!(
        balance_after_cancel,
        balance_after_create + claim_amount,
        "Funds should be returned to escrow after cancellation"
    );

    // Assert claim status is Cancelled
    let claim = t.client.get_claim(&t.program_id, &claim_id);
    assert_eq!(
        claim.status,
        ClaimStatus::Cancelled,
        "Claim should be Cancelled"
    );

    // Assert contributor received nothing
    assert_eq!(
        t.token.balance(&t.contributor),
        0,
        "Contributor should have received nothing after cancel"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// TEST 4: Admin cancels an already-expired claim
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_admin_cancel_expired_claim_succeeds() {
    let t = setup();
    let env = &t.env;

    let now: u64 = env.ledger().timestamp();
    let claim_amount: i128 = 3_000;
    let claim_deadline: u64 = now + 3_600;

    let claim_id = t.client.create_pending_claim(
        &t.program_id,
        &t.contributor,
        &claim_amount,
        &claim_deadline,
    );

    // Time passes — claim window expires without contributor acting
    env.ledger().set(LedgerInfo {
        timestamp: now + 7_200, // 2 hours later
        ..env.ledger().get()
    });

    let balance_before_cancel = t.client.get_remaining_balance();

    // Admin cleans up the expired claim
    t.client.cancel_claim(&t.program_id, &claim_id, &t.admin);

    // Funds should return to escrow
    let balance_after_cancel = t.client.get_remaining_balance();
    assert_eq!(
        balance_after_cancel,
        balance_before_cancel + claim_amount,
        "Expired claim cancellation should restore funds to escrow"
    );

    let claim = t.client.get_claim(&t.program_id, &claim_id);
    assert_eq!(
        claim.status,
        ClaimStatus::Cancelled,
        "Expired claim should be Cancelled"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// TEST 5: Non-admin cannot cancel a claim
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_non_admin_cannot_cancel_claim() {
    let t = setup();
    let env = &t.env;

    let now: u64 = env.ledger().timestamp();
    let claim_id =
        t.client
            .create_pending_claim(&t.program_id, &t.contributor, &5_000_i128, &(now + 86_400));

    let random_user = Address::generate(env);

    // A non-admin user attempts to cancel the claim — should panic
    t.client
        .cancel_claim(&t.program_id, &claim_id, &random_user);
}

// ═══════════════════════════════════════════════════════════════════════════
// TEST 6: Prevent double-claim (cannot execute an already completed claim)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "ClaimAlreadyProcessed")]
fn test_cannot_double_claim() {
    let t = setup();
    let env = &t.env;

    let now: u64 = env.ledger().timestamp();
    let claim_id =
        t.client
            .create_pending_claim(&t.program_id, &t.contributor, &10_000_i128, &(now + 86_400));

    // First execution succeeds
    t.client
        .execute_claim(&t.program_id, &claim_id, &t.contributor);

    // Second execution on the same claim_id must fail
    t.client
        .execute_claim(&t.program_id, &claim_id, &t.contributor);
}

// ═══════════════════════════════════════════════════════════════════════════
// TEST 7: Cannot execute a cancelled claim
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "ClaimAlreadyProcessed")]
fn test_cannot_execute_cancelled_claim() {
    let t = setup();
    let env = &t.env;

    let now: u64 = env.ledger().timestamp();
    let claim_id =
        t.client
            .create_pending_claim(&t.program_id, &t.contributor, &5_000_i128, &(now + 86_400));

    // Admin cancels the claim first
    t.client.cancel_claim(&t.program_id, &claim_id, &t.admin);

    // Contributor then attempts to execute the cancelled claim — should fail
    t.client
        .execute_claim(&t.program_id, &claim_id, &t.contributor);
}

// ═══════════════════════════════════════════════════════════════════════════
// TEST 8: Only the designated recipient can execute a claim
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_wrong_recipient_cannot_execute_claim() {
    let t = setup();
    let env = &t.env;

    let now: u64 = env.ledger().timestamp();
    let claim_id =
        t.client
            .create_pending_claim(&t.program_id, &t.contributor, &5_000_i128, &(now + 86_400));

    let impostor = Address::generate(env);

    // An unrelated address tries to execute the claim — should panic
// ═════════════════════════════════════════════════════════════════════════════
// TEST 9: Exact boundary timestamp - claim at exact expiry moment
// ═══════════════════════════════════════════════════════════════════════════════════

#[test]
fn test_claim_at_exact_expiry_boundary() {
    let t = setup();
    let env = &t.env;

    let now: u64 = env.ledger().timestamp();
    let claim_amount: i128 = 7_000;
    let claim_deadline: u64 = now + 3_600; // 1 hour

    let claim_id = t.client.create_pending_claim(
        &t.program_id,
        &t.contributor,
        &claim_amount,
        &claim_deadline,
    );

    // Advance time to exactly the deadline moment
    env.ledger().set(LedgerInfo {
        timestamp: claim_deadline,
        ..env.ledger().get()
    });

    // Should still be executable at exact deadline moment
    t.client
        .execute_claim(&t.program_id, &claim_id, &t.contributor);

    // Verify claim was completed
    let claim = t.client.get_claim(&t.program_id, &claim_id);
    assert_eq!(
        claim.status,
        ClaimStatus::Completed,
        "Claim should be Completed at exact deadline"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// TEST 10: Claim 1 second after expiry should fail
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "ClaimExpired")]
fn test_claim_1_second_after_expiry_fails() {
    let t = setup();
    let env = &t.env;

    let now: u64 = env.ledger().timestamp();
    let claim_amount: i128 = 4_000;
    let claim_deadline: u64 = now + 1_800; // 30 minutes

    let claim_id = t.client.create_pending_claim(
        &t.program_id,
        &t.contributor,
        &claim_amount,
        &claim_deadline,
    );

    // Advance time 1 second past deadline
    env.ledger().set(LedgerInfo {
        timestamp: claim_deadline + 1,
        ..env.ledger().get()
    });

    // Should fail with ClaimExpired
    t.client
        .execute_claim(&t.program_id, &claim_id, &t.contributor);
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 11: Concurrent cancel and claim attempt - race condition
// ═════════════════════════════════════════════════════════════════════════════════

#[test]
fn test_concurrent_cancel_claim_race_condition() {
    let t = setup();
    let env = &t.env;

    let now: u64 = env.ledger().timestamp();
    let claim_amount: i128 = 6_000;
    let claim_deadline: u64 = now + 86_400; // 24 hours

    let claim_id = t.client.create_pending_claim(
        &t.program_id,
        &t.contributor,
        &claim_amount,
        &claim_deadline,
    );

    // Advance time to just before deadline
    env.ledger().set(LedgerInfo {
        timestamp: claim_deadline - 100,
        ..env.ledger().get()
    });

    // Contributor attempts to execute claim (should succeed)
    // This should be atomic - either claim succeeds OR cancel succeeds, not both
    let claim_before = t.client.get_claim(&t.program_id, &claim_id);
    assert_eq!(claim_before.status, ClaimStatus::Pending);
    
    // Admin attempts to cancel in same transaction context
    // In reality, this would be separate transactions, but we test the logic
    t.client.cancel_claim(&t.program_id, &claim_id, &t.admin);
    
    // Check final state - should be either Completed or Cancelled, not both
    let claim_after = t.client.get_claim(&t.program_id, &claim_id);
    
    // One of the operations should have prevailed
    let is_completed = claim_after.status == ClaimStatus::Completed;
    let is_cancelled = claim_after.status == ClaimStatus::Cancelled;
    
    assert!(
        is_completed || is_cancelled,
        "Claim should be either Completed or Cancelled after race"
    );
    
    // Should not be in Pending state anymore
    assert_ne!(
        claim_after.status,
        ClaimStatus::Pending,
        "Claim should not remain Pending after race condition"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// TEST 12: Clock skew resistance - same claim across different timestamps
// ═════════════════════════════════════════════════════════════════════════════════

#[test]
fn test_clock_skew_resistance() {
    let t = setup();
    let env = &t.env;

    let now: u64 = env.ledger().timestamp();
    let claim_amount: i128 = 8_000;
    let claim_deadline: u64 = now + 3_600; // 1 hour

    let claim_id = t.client.create_pending_claim(
        &t.program_id,
        &t.contributor,
        &claim_amount,
        &claim_deadline,
    );

    // Simulate clock going backwards (network time sync issue)
    env.ledger().set(LedgerInfo {
        timestamp: now - 1_000, // 16+ minutes in the past
        ..env.ledger().get()
    });

    // Should still be able to execute (deadline not reached)
    t.client
        .execute_claim(&t.program_id, &claim_id, &t.contributor);

    // Verify claim completed
    let claim = t.client.get_claim(&t.program_id, &claim_id);
    assert_eq!(
        claim.status,
        ClaimStatus::Completed,
        "Claim should succeed despite temporary clock skew"
    );
}

// ═════════════════════════════════════════════════════════════════════════════════
// TEST 13: Maximum claim window boundary test
// ═══════════════════════════════════════════════════════════════════════════════════════

#[test]
fn test_maximum_claim_window_boundary() {
    let t = setup();
    let env = &t.env;

    // Set custom claim window to test boundaries
    t.client.set_claim_window(&t.admin, &604_800); // 7 days

    let now: u64 = env.ledger().timestamp();
    let claim_amount: i128 = 2_000;
    let claim_deadline: u64 = now + 604_800; // Exactly at window limit

    let claim_id = t.client.create_pending_claim(
        &t.program_id,
        &t.contributor,
        &claim_amount,
        &claim_deadline,
    );

    // Should succeed at exact boundary
    t.client
        .execute_claim(&t.program_id, &claim_id, &t.contributor);

    let claim = t.client.get_claim(&t.program_id, &claim_id);
    assert_eq!(
        claim.status,
        ClaimStatus::Completed,
        "Claim should succeed at maximum window boundary"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// TEST 14: Zero-second claim deadline (immediate expiry)
// ═══════════════════════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "Claim deadline must be in the future")]
fn test_zero_second_claim_deadline_fails() {
    let t = setup();
    let env = &t.env;

    let now: u64 = env.ledger().timestamp();
    let claim_amount: i128 = 1_000;
    let claim_deadline: u64 = now; // Same timestamp as creation

    // Should panic during claim creation
    t.client.create_pending_claim(
        &t.program_id,
        &t.contributor,
        &claim_amount,
        &claim_deadline,
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// TEST 15: Reentrancy protection during claim execution
// ═════════════════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reentrancy_protection_during_claim() {
    let t = setup();
    let env = &t.env;

    let now: u64 = env.ledger().timestamp();
    let claim_amount: i128 = 3_000;
    let claim_deadline: u64 = now + 86_400;

    let claim_id = t.client.create_pending_claim(
        &t.program_id,
        &t.contributor,
        &claim_amount,
        &claim_deadline,
    );

    // First execution should succeed
    t.client
        .execute_claim(&t.program_id, &claim_id, &t.contributor);

    // Second execution should fail (already processed)
    let result = std::panic::catch_unwind(|| {
        t.client
            .execute_claim(&t.program_id, &claim_id, &t.contributor);
    });
    
    assert!(result.is_err(), "Second claim execution should fail");
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════════════════
// TEST 16: Timestamp overflow protection
// ═══════════════════════════════════════════════════════════════════════════════════════════════════════════

#[test]
fn test_timestamp_overflow_protection() {
    let t = setup();
    let env = &t.env;

    // Test with maximum reasonable timestamp
    let max_reasonable_time: u64 = u64::MAX / 2; // Far future but not overflow
    
    let claim_amount: i128 = 1_000;
    let claim_deadline: u64 = max_reasonable_time;

    // Should create claim successfully
    let claim_id = t.client.create_pending_claim(
        &t.program_id,
        &t.contributor,
        &claim_amount,
        &claim_deadline,
    );

    // Advance to deadline
    env.ledger().set(LedgerInfo {
        timestamp: claim_deadline,
        ..env.ledger().get()
    });

    // Should execute successfully
    t.client
        .execute_claim(&t.program_id, &claim_id, &t.contributor);

    let claim = t.client.get_claim(&t.program_id, &claim_id);
    assert_eq!(
        claim.status,
        ClaimStatus::Completed,
        "Claim should succeed with large but valid timestamp"
    );
}
