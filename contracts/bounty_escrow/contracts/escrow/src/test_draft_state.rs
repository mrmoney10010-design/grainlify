#![cfg(test)]

/// # Draft State Tests for Bounty Escrow
///
/// This module tests the draft state functionality where escrows
/// are created in Draft status and must be explicitly published
/// before funds can be released or refunded.
use crate::test::setup_test_environment;
use soroban_sdk::{Address, Env};

#[test]
fn test_escrow_starts_in_draft_status() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _contract_id, admin, depositor, token_id, _token_admin) =
        setup_test_environment(&env);

    let bounty_id = 1u64;
    let amount = 1000i128;
    let deadline = env.ledger().timestamp() + 1000;

    // Lock funds - should create escrow in Draft status
    client.lock_funds(&depositor, &bounty_id, &amount, &deadline);

    // Verify escrow is in Draft status
    let info = client.get_escrow_info(&bounty_id);
    assert_eq!(info.status as u32, 0); // Draft is first enum variant (0)
}

#[test]
#[should_panic(expected = "InvalidState")]
fn test_release_fails_in_draft_status() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _contract_id, _admin, depositor, _token_id, _token_admin) =
        setup_test_environment(&env);

    let bounty_id = 2u64;
    let amount = 1000i128;
    let deadline = env.ledger().timestamp() + 1000;
    let contributor = Address::generate(&env);

    // Lock funds - creates Draft escrow
    client.lock_funds(&depositor, &bounty_id, &amount, &deadline);

    // Try to release - should fail because escrow is in Draft
    client.release_funds(&bounty_id, &contributor);
}

#[test]
#[should_panic(expected = "InvalidState")]
fn test_refund_fails_in_draft_status() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _contract_id, _admin, depositor, _token_id, _token_admin) =
        setup_test_environment(&env);

    let bounty_id = 3u64;
    let amount = 1000i128;
    let deadline = env.ledger().timestamp() + 1000;

    // Lock funds - creates Draft escrow
    client.lock_funds(&depositor, &bounty_id, &amount, &deadline);

    // Advance time past deadline
    env.ledger().set_timestamp(deadline + 1);

    // Try to refund - should fail because escrow is in Draft
    client.refund(&bounty_id);
}

#[test]
fn test_publish_transitions_to_locked() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _contract_id, _admin, depositor, _token_id, _token_admin) =
        setup_test_environment(&env);

    let bounty_id = 4u64;
    let amount = 1000i128;
    let deadline = env.ledger().timestamp() + 1000;

    // Lock funds - creates Draft escrow
    client.lock_funds(&depositor, &bounty_id, &amount, &deadline);

    // Verify initial Draft status
    let info = client.get_escrow_info(&bounty_id);
    assert_eq!(info.status as u32, 0); // Draft

    // Publish the escrow
    client.publish(&bounty_id);

    // Verify transition to Locked status
    let info = client.get_escrow_info(&bounty_id);
    assert_eq!(info.status as u32, 1); // Locked is second variant (1)
}

#[test]
fn test_release_succeeds_after_publish() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _contract_id, _admin, depositor, _token_id, _token_admin) =
        setup_test_environment(&env);

    let bounty_id = 5u64;
    let amount = 1000i128;
    let deadline = env.ledger().timestamp() + 1000;
    let contributor = Address::generate(&env);

    // Lock funds - creates Draft escrow
    client.lock_funds(&depositor, &bounty_id, &amount, &deadline);

    // Publish the escrow
    client.publish(&bounty_id);

    // Release should now succeed
    client.release_funds(&bounty_id, &contributor);

    // Verify status changed to Released
    let info = client.get_escrow_info(&bounty_id);
    assert_eq!(info.status as u32, 2); // Released
}

#[test]
fn test_refund_succeeds_after_publish() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _contract_id, _admin, depositor, _token_id, _token_admin) =
        setup_test_environment(&env);

    let bounty_id = 6u64;
    let amount = 1000i128;
    let deadline = env.ledger().timestamp() + 1000;

    // Lock funds - creates Draft escrow
    client.lock_funds(&depositor, &bounty_id, &amount, &deadline);

    // Publish the escrow
    client.publish(&bounty_id);

    // Advance time past deadline
    env.ledger().set_timestamp(deadline + 1);

    // Refund should now succeed
    client.refund(&bounty_id);

    // Verify status changed to Refunded
    let info = client.get_escrow_info(&bounty_id);
    assert_eq!(info.status as u32, 3); // Refunded
}

#[test]
#[should_panic(expected = "InvalidState")]
fn test_publish_fails_if_already_locked() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _contract_id, _admin, depositor, _token_id, _token_admin) =
        setup_test_environment(&env);

    let bounty_id = 7u64;
    let amount = 1000i128;
    let deadline = env.ledger().timestamp() + 1000;

    // Lock funds - creates Draft escrow
    client.lock_funds(&depositor, &bounty_id, &amount, &deadline);

    // Publish once - should succeed
    client.publish(&bounty_id);

    // Try to publish again - should fail
    client.publish(&bounty_id);
}

#[test]
#[should_panic(expected = "BountyNotFound")]
fn test_publish_fails_for_nonexistent_bounty() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _contract_id, _admin, _depositor, _token_id, _token_admin) =
        setup_test_environment(&env);

    let nonexistent_bounty_id = 999u64;

    // Try to publish non-existent bounty - should fail
    client.publish(&nonexistent_bounty_id);
}
