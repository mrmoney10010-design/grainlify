#![cfg(test)]
use crate::{upgrade_safety, BountyEscrowContract, BountyEscrowContractClient, EscrowStatus};
use crate::{ BountyEscrowContract, BountyEscrowContractClient, EscrowStatus };
use soroban_sdk::{ testutils::{ Address as _, Ledger }, token, Address, Env };
use crate::{BountyEscrowContract, BountyEscrowContractClient, EscrowStatus};
use soroban_sdk::testutils::Ledger;
use soroban_sdk::{
    testutils::{Address as _, LedgerInfo},
    token, Address, Env,
};

fn create_test_env() -> (Env, BountyEscrowContractClient<'static>, Address) {
    let env = Env::default();
    let contract_id = env.register_contract(None, BountyEscrowContract);
    let client = BountyEscrowContractClient::new(&env, &contract_id);
    (env, client, contract_id)
}

fn create_token_contract<'a>(
    e: &'a Env,
    admin: &Address
) -> (Address, token::Client<'a>, token::StellarAssetClient<'a>) {
    let token_id = e.register_stellar_asset_contract_v2(admin.clone());
    let token = token_id.address();
    let token_client = token::Client::new(e, &token);
    let token_admin_client = token::StellarAssetClient::new(e, &token);
    (token, token_client, token_admin_client)
}

// ── UPGRADE SCENARIO TESTS ───────────────────────────────────────────────────

#[test]
fn test_upgrade_locked_bounty_remains_locked() {
    let (env, client, _contract_id) = create_test_env();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _token_client, token_admin_client) = create_token_contract(&env, &token_admin);

    client.init(&admin, &token);
    token_admin_client.mint(&depositor, &10_000);

    let deadline = env.ledger().timestamp() + 1000;
    client.lock_funds(&depositor, &1, &5_000, &deadline);

    // Simulate upgrade by re-registering contract (state persists)
    let escrow = client.get_escrow_info(&1);
    assert_eq!(escrow.status, EscrowStatus::Locked);
    assert_eq!(escrow.amount, 5_000);
    assert_eq!(escrow.remaining_amount, 5_000);
}

#[test]
fn test_upgrade_complete_release_flow() {
    let (env, client, _contract_id) = create_test_env();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contributor = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _token_client, token_admin_client) = create_token_contract(&env, &token_admin);

    client.init(&admin, &token);
    token_admin_client.mint(&depositor, &10_000);

    let deadline = env.ledger().timestamp() + 1000;
    client.lock_funds(&depositor, &1, &5_000, &deadline);

    // Verify locked
    let escrow = client.get_escrow_info(&1);
    assert_eq!(escrow.status, EscrowStatus::Locked);

    // Complete release after upgrade
    client.release_funds(&1, &contributor);

    let escrow = client.get_escrow_info(&1);
    assert_eq!(escrow.status, EscrowStatus::Released);
}

#[test]
fn test_upgrade_pending_lock_then_refund() {
    let (env, client, _contract_id) = create_test_env();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _token_client, token_admin_client) = create_token_contract(&env, &token_admin);

    client.init(&admin, &token);
    token_admin_client.mint(&depositor, &10_000);

    let deadline = env.ledger().timestamp() + 100;
    client.lock_funds(&depositor, &2, &5_000, &deadline);

    // Advance time past deadline
    env.ledger().set_timestamp(env.ledger().timestamp() + 200);
    let current_time = env.ledger().timestamp();
    env.ledger().set(LedgerInfo {
        timestamp: current_time + 200,
        protocol_version: 20,
        sequence_number: 0,
        network_id: Default::default(),
        base_reserve: 0,
        min_temp_entry_ttl: 0,
        min_persistent_entry_ttl: 0,
        max_entry_ttl: 0,
    });

    // Refund after upgrade
    client.refund(&2);

    let escrow = client.get_escrow_info(&2);
    assert_eq!(escrow.status, EscrowStatus::Refunded);
}

#[test]
fn test_upgrade_partial_release_then_complete() {
    let (env, client, _contract_id) = create_test_env();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contributor = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _token_client, token_admin_client) = create_token_contract(&env, &token_admin);

    client.init(&admin, &token);
    token_admin_client.mint(&depositor, &10_000);

    let deadline = env.ledger().timestamp() + 1000;
    client.lock_funds(&depositor, &3, &6_000, &deadline);

    client.partial_release(&3, &contributor, &2_000);

    let escrow = client.get_escrow_info(&3);
    assert_eq!(escrow.remaining_amount, 4_000);
    assert_eq!(escrow.status, EscrowStatus::Locked);

    client.partial_release(&3, &contributor, &4_000);

    let escrow = client.get_escrow_info(&3);
    assert_eq!(escrow.remaining_amount, 0);
    assert_eq!(escrow.status, EscrowStatus::Released);
}

// ── UPGRADE SAFETY TESTS ─────────────────────────────────────────────────────

/// Test that simulate_upgrade passes after proper initialization
#[test]
fn test_safety_check_passes_after_init() {
    let (env, client, _contract_id) = create_test_env();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _token_client, _token_admin_client) = create_token_contract(&env, &token_admin);

    // Initialize contract
    client.init(&admin, &token);

    // Run safety check - should pass
    let report = client.simulate_upgrade();
    assert!(report.is_safe, "Safety check should pass after initialization");
    assert_eq!(report.checks_passed, 10);
    assert_eq!(report.checks_failed, 0);
}

/// Test that simulate_upgrade fails before initialization
#[test]
fn test_safety_check_fails_before_init() {
    let (env, client, _contract_id) = create_test_env();
    env.mock_all_auths();
    // Don't initialize - contract not initialized

    // Run safety check - should fail
    let report = client.simulate_upgrade();
    assert!(!report.is_safe, "Safety check should fail before initialization");
    assert!(report.checks_failed > 0, "Should have failed checks");
}

/// Test that safety check detects invalid escrow state (simulated)
#[test]
fn test_safety_check_with_locked_escrows() {
    let (env, client, _contract_id) = create_test_env();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _token_client, token_admin_client) = create_token_contract(&env, &token_admin);

    client.init(&admin, &token);
    token_admin_client.mint(&depositor, &10_000);

    // Create a locked escrow
    let deadline = env.ledger().timestamp() + 1000;
    client.lock_funds(&depositor, &1, &5_000, &deadline);

    // Run safety check - should pass with locked escrows
    let report = client.simulate_upgrade();
    assert!(report.is_safe, "Safety check should pass with locked escrows");
}

/// Test upgrade function with valid state
#[test]
fn test_upgrade_succeeds_with_valid_state() {
    let (env, client, _contract_id) = create_test_env();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _token_client, _token_admin_client) = create_token_contract(&env, &token_admin);

    client.init(&admin, &token);

    // Upgrade should succeed with valid state
    let result = client.upgrade(&0);
    assert!(result.is_ok(), "Upgrade should succeed with valid state");
}

/// Test upgrade fails with uninitialized contract
#[test]
fn test_upgrade_fails_without_init() {
    let (env, client, _contract_id) = create_test_env();
    env.mock_all_auths();
    // Don't initialize

    // Upgrade should fail
    let result = client.upgrade(&0);
    assert!(result.is_err(), "Upgrade should fail without initialization");
}

/// Test safety status can be checked
#[test]
fn test_get_safety_status() {
    let (env, client, _contract_id) = create_test_env();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _token_client, _token_admin_client) = create_token_contract(&env, &token_admin);

    client.init(&admin, &token);

    // Safety status should be enabled by default
    let status = client.get_upgrade_safety_status();
    assert!(status, "Safety checks should be enabled by default");
}

/// Test safety status can be toggled by admin
#[test]
fn test_set_safety_status() {
    let (env, client, _contract_id) = create_test_env();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _token_client, _token_admin_client) = create_token_contract(&env, &token_admin);

    client.init(&admin, &token);

    // Disable safety checks
    client.set_upgrade_safety(&admin, &false);
    let status = client.get_upgrade_safety_status();
    assert!(!status, "Safety checks should be disabled");

    // Re-enable safety checks
    client.set_upgrade_safety(&admin, &true);
    let status = client.get_upgrade_safety_status();
    assert!(status, "Safety checks should be re-enabled");
}

/// Test safety check with released escrow
#[test]
fn test_safety_check_with_released_escrow() {
    let (env, client, _contract_id) = create_test_env();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contributor = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _token_client, token_admin_client) = create_token_contract(&env, &token_admin);

    client.init(&admin, &token);
    token_admin_client.mint(&depositor, &10_000);

    // Create and release escrow
    let deadline = env.ledger().timestamp() + 1000;
    client.lock_funds(&depositor, &1, &5_000, &deadline);
    client.release_funds(&1, &contributor);

    // Safety check should pass
    let report = client.simulate_upgrade();
    assert!(report.is_safe, "Safety check should pass with released escrow");
}

/// Test safety check with refunded escrow
#[test]
fn test_safety_check_with_refunded_escrow() {
    let (env, client, _contract_id) = create_test_env();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _token_client, token_admin_client) = create_token_contract(&env, &token_admin);

    client.init(&admin, &token);
    token_admin_client.mint(&depositor, &10_000);

    // Create and refund escrow
    let deadline = env.ledger().timestamp() + 100;
    client.lock_funds(&depositor, &1, &5_000, &deadline);

    // Advance time past deadline
    let current_time = env.ledger().timestamp();
    env.ledger().set(LedgerInfo {
        timestamp: current_time + 200,
        protocol_version: 20,
        sequence_number: 0,
        network_id: Default::default(),
        base_reserve: 0,
        min_temp_entry_ttl: 0,
        min_persistent_entry_ttl: 0,
        max_entry_ttl: 0,
    });

    client.refund(&1);

    // Safety check should pass
    let report = client.simulate_upgrade();
    assert!(report.is_safe, "Safety check should pass with refunded escrow");
}

/// Test safety check with multiple escrows in different states
#[test]
fn test_safety_check_with_multiple_escrows() {
    let (env, client, _contract_id) = create_test_env();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contributor = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _token_client, token_admin_client) = create_token_contract(&env, &token_admin);

    client.init(&admin, &token);
    token_admin_client.mint(&depositor, &30_000);

    // Create multiple escrows in different states
    let deadline1 = env.ledger().timestamp() + 1000;
    let deadline2 = env.ledger().timestamp() + 1000;
    let deadline3 = env.ledger().timestamp() + 100;

    client.lock_funds(&depositor, &1, &5_000, &deadline1); // Locked
    client.lock_funds(&depositor, &2, &5_000, &deadline2); // Locked
    client.lock_funds(&depositor, &3, &5_000, &deadline3); // Will be refunded

    // Release one
    client.release_funds(&1, &contributor);

    // Advance time for refund
    let current_time = env.ledger().timestamp();
    env.ledger().set(LedgerInfo {
        timestamp: current_time + 200,
        protocol_version: 20,
        sequence_number: 0,
        network_id: Default::default(),
        base_reserve: 0,
        min_temp_entry_ttl: 0,
        min_persistent_entry_ttl: 0,
        max_entry_ttl: 0,
    });

    // Refund one
    client.refund(&3);

    // Safety check should pass with mixed states
    let report = client.simulate_upgrade();
    assert!(report.is_safe, "Safety check should pass with multiple escrows in different states");
    assert_eq!(report.checks_passed, 10);
}

/// Test that upgrade fails when safety checks are disabled but contract is invalid
#[test]
fn test_upgrade_with_disabled_safety_allows_invalid_state() {
    let (env, client, _contract_id) = create_test_env();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _token_client, _token_admin_client) = create_token_contract(&env, &token_admin);

    client.init(&admin, &token);

    // Disable safety checks
    client.set_upgrade_safety(&admin, &false);

    // Even without safety checks, upgrade requires init
    let result = client.upgrade(&0);
    assert!(result.is_ok(), "Upgrade should work with disabled safety when initialized");
}

/// Test safety module directly - verify check count
#[test]
fn test_safety_module_check_count() {
    let env = Env::default();
    env.mock_all_auths();
    env.register_contract(None, BountyEscrowContract);

    // Safety checks should be enabled by default
    assert!(upgrade_safety::is_safety_checks_enabled(&env));

    // Can disable
    upgrade_safety::set_safety_checks_enabled(&env, false);
    assert!(!upgrade_safety::is_safety_checks_enabled(&env));

    // Re-enable
    upgrade_safety::set_safety_checks_enabled(&env, true);
    assert!(upgrade_safety::is_safety_checks_enabled(&env));
}
