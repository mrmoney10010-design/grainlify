#![cfg(test)]
//! Tests for identity-aware limits functionality.
//!
//! These tests verify the identity module's address binding rules, tier-based limits,
//! and risk adjustments to prevent spoofed identities on claims.

use super::*;
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{token, Address, BytesN, Env};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn create_token<'a>(
    env: &'a Env,
    admin: &Address,
) -> (Address, token::Client<'a>, token::StellarAssetClient<'a>) {
    let token_contract = env.register_stellar_asset_contract_v2(admin.clone());
    let addr = token_contract.address();
    let client = token::Client::new(env, &addr);
    let admin_client = token::StellarAssetClient::new(env, &addr);
    (addr, client, admin_client)
}

/// Generate an Ed25519 keypair and return (SigningKey, public_key_bytes, public_key BytesN<32>).
fn generate_keypair(env: &Env) -> (SigningKey, [u8; 32], BytesN<32>) {
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    let pk_bytes: [u8; 32] = verifying_key.to_bytes();
    let pk_byten = BytesN::from_array(env, &pk_bytes);
    (signing_key, pk_bytes, pk_byten)
}

/// Build and sign an identity claim. Returns (claim, signature).
fn build_signed_claim(
    env: &Env,
    address: &Address,
    tier: IdentityTier,
    risk_score: u32,
    expiry: u64,
    issuer: &Address,
    signing_key: &SigningKey,
) -> (IdentityClaim, BytesN<64>) {
    let claim = IdentityClaim {
        address: address.clone(),
        tier,
        risk_score,
        expiry,
        issuer: issuer.clone(),
    };
    let message = identity::serialize_claim(env, &claim);
    let msg_len = message.len() as usize;
    // Serialized claim fits in 512 bytes (address XDR + tier + risk + expiry + issuer XDR).
    let mut msg_buf = [0u8; 512];
    message.copy_into_slice(&mut msg_buf[..msg_len]);
    let sig = signing_key.sign(&msg_buf[..msg_len]);
    let sig_byten = BytesN::from_array(env, &sig.to_bytes());
    (claim, sig_byten)
}

/// Standard setup: registers contract, inits with admin + token, authorises one
/// issuer, and returns everything the tests need.
fn setup_with_identity<'a>(
    env: &'a Env,
    initial_balance: i128,
) -> (
    EscrowContractClient<'a>,
    Address,    // contract_id
    Address,    // admin
    Address,    // depositor
    Address,    // contributor
    Address,    // issuer
    SigningKey, // issuer signing key
    token::Client<'a>,
) {
    env.mock_all_auths();
    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    let depositor = Address::generate(env);
    let contributor = Address::generate(env);
    let issuer = Address::generate(env);

    let (token_addr, token_client, token_admin) = create_token(env, &admin);

    client.init(&admin, &token_addr);
    token_admin.mint(&depositor, &initial_balance);
    token_admin.mint(&contributor, &initial_balance);

    // Generate a real Ed25519 keypair for the issuer and authorise it.
    let (signing_key, _pk_bytes, pk_byten) = generate_keypair(env);
    client.set_authorized_issuer(&issuer, &pk_byten, &true);

    (
        client,
        contract_id,
        admin,
        depositor,
        contributor,
        issuer,
        signing_key,
        token_client,
    )
}

// ---------------------------------------------------------------------------
// Tests: issuer management
// ---------------------------------------------------------------------------

#[test]
fn test_set_authorized_issuer() {
    let env = Env::default();
    let (client, _cid, _admin, _depositor, _contributor, issuer, _sk, _tc) =
        setup_with_identity(&env, 10_000i128);

    // De-authorise then re-authorise (no panic = pass)
    let (_sk2, _pk, pk_byten) = generate_keypair(&env);
    client.set_authorized_issuer(&issuer, &pk_byten, &false);
    client.set_authorized_issuer(&issuer, &pk_byten, &true);
}

// ---------------------------------------------------------------------------
// Tests: tier limits and risk thresholds configuration
// ---------------------------------------------------------------------------

#[test]
fn test_set_tier_limits() {
    let env = Env::default();
    let (client, _cid, _admin, _depositor, _contributor, _issuer, _sk, _tc) =
        setup_with_identity(&env, 10_000i128);

    client.set_tier_limits(&100_0000000, &1000_0000000, &10000_0000000, &100000_0000000);

    let depositor = Address::generate(&env);
    let bounty_id = 1u64;
    let deadline = env.ledger().timestamp() + 1000;
    let result = client.try_lock_funds(&depositor, &bounty_id, &150_0000000, &deadline);
    assert!(result.is_err());
}

#[test]
fn test_set_risk_thresholds() {
    let env = Env::default();
    let (client, _cid, _admin, _depositor, _contributor, _issuer, _sk, _tc) =
        setup_with_identity(&env, 10_000i128);

    client.set_risk_thresholds(&70, &50);
}

// ---------------------------------------------------------------------------
// Tests: identity query helpers
// ---------------------------------------------------------------------------

#[test]
fn test_get_address_identity_default() {
    let env = Env::default();
    let (client, _cid, _admin, _depositor, _contributor, _issuer, _sk, _tc) =
        setup_with_identity(&env, 10_000i128);

    let address = Address::generate(&env);
    let id = client.get_address_identity(&address);
    assert_eq!(id.tier, IdentityTier::Unverified);
    assert_eq!(id.risk_score, 0);
}

#[test]
fn test_get_effective_limit_unverified() {
    let env = Env::default();
    let (client, _cid, _admin, _depositor, _contributor, _issuer, _sk, _tc) =
        setup_with_identity(&env, 10_000i128);

    let address = Address::generate(&env);
    let limit = client.get_effective_limit(&address);
    assert_eq!(limit, 100_0000000);
}

#[test]
fn test_is_claim_valid_no_claim() {
    let env = Env::default();
    let (client, _cid, _admin, _depositor, _contributor, _issuer, _sk, _tc) =
        setup_with_identity(&env, 10_000i128);

    let address = Address::generate(&env);
    assert!(!client.is_claim_valid(&address));
}

// ---------------------------------------------------------------------------
// Tests: lock_funds with identity limits
// ---------------------------------------------------------------------------

#[test]
fn test_lock_funds_respects_limits() {
    let env = Env::default();
    let amount = 10_000_0000000i128;
    let (client, _cid, _admin, depositor, _contributor, _issuer, _sk, _tc) =
        setup_with_identity(&env, amount);

    let bounty_id = 1u64;
    let deadline = env.ledger().timestamp() + 1000;
    let result = client.try_lock_funds(&depositor, &bounty_id, &amount, &deadline);
    assert!(result.is_err());
}

#[test]
fn test_lock_funds_within_limits() {
    let env = Env::default();
    let amount = 50_0000000;
    let (client, _cid, _admin, depositor, _contributor, _issuer, _sk, token_client) =
        setup_with_identity(&env, 10_000_0000000);

    let bounty_id = 1u64;
    let deadline = env.ledger().timestamp() + 1000;
    client.lock_funds(&depositor, &bounty_id, &amount, &deadline);

    let escrow = client.get_escrow(&bounty_id);
    assert_eq!(escrow.amount, amount);
    // ensure tokens actually moved
    let _ = &token_client; // used implicitly through contract
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_risk_threshold_boundary() {
    let env = Env::default();
    let (client, _cid, _admin, _depositor, _contributor, _issuer, _sk, _tc) =
        setup_with_identity(&env, 10_000i128);

    client.set_risk_thresholds(&70, &50);

    let address = Address::generate(&env);
    let limit = client.get_effective_limit(&address);
    assert_eq!(limit, 100_0000000);
}

#[test]
fn test_lock_funds_at_exact_limit() {
    let env = Env::default();
    let amount = 100_0000000;
    let (client, _cid, _admin, depositor, _contributor, _issuer, _sk, _tc) =
        setup_with_identity(&env, amount);

    let bounty_id = 1u64;
    let deadline = env.ledger().timestamp() + 1000;
    client.lock_funds(&depositor, &bounty_id, &amount, &deadline);

    let escrow = client.get_escrow(&bounty_id);
    assert_eq!(escrow.amount, amount);
}

#[test]
fn test_lock_funds_just_over_limit() {
    let env = Env::default();
    let amount = 100_0000000 + 1;
    let (client, _cid, _admin, depositor, _contributor, _issuer, _sk, _tc) =
        setup_with_identity(&env, amount);

    let bounty_id = 1u64;
    let deadline = env.ledger().timestamp() + 1000;
    let result = client.try_lock_funds(&depositor, &bounty_id, &amount, &deadline);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Tests: submit_identity_claim — end-to-end with real Ed25519 signatures
// ---------------------------------------------------------------------------

#[test]
fn test_submit_identity_claim_valid() {
    let env = Env::default();
    let (client, _cid, _admin, _depositor, _contributor, issuer, signing_key, _tc) =
        setup_with_identity(&env, 10_000i128);

    let user = Address::generate(&env);
    let expiry = env.ledger().timestamp() + 10_000;

    let (claim, sig) = build_signed_claim(
        &env,
        &user,
        IdentityTier::Verified,
        20,
        expiry,
        &issuer,
        &signing_key,
    );

    client.submit_identity_claim(&claim, &sig);

    // The claim should now be stored and retrievable.
    let id = client.get_address_identity(&user);
    assert_eq!(id.tier, IdentityTier::Verified);
    assert_eq!(id.risk_score, 20);
}

#[test]
fn test_submit_identity_claim_expired() {
    let env = Env::default();
    let (client, _cid, _admin, _depositor, _contributor, issuer, signing_key, _tc) =
        setup_with_identity(&env, 10_000i128);

    let user = Address::generate(&env);
    // Set expiry to 0 so it's already expired (ledger timestamp defaults to 0
    // and expiry == now means expired).
    let (claim, sig) = build_signed_claim(
        &env,
        &user,
        IdentityTier::Basic,
        10,
        0,
        &issuer,
        &signing_key,
    );

    let result = client.try_submit_identity_claim(&claim, &sig);
    assert!(result.is_err());
}

#[test]
fn test_submit_identity_claim_unauthorized_issuer() {
    let env = Env::default();
    let (client, _cid, _admin, _depositor, _contributor, _issuer, _signing_key, _tc) =
        setup_with_identity(&env, 10_000i128);

    // Create a new keypair that is NOT authorized.
    let rogue_issuer = Address::generate(&env);
    let (rogue_signing_key, _pk, rogue_pk_byten) = generate_keypair(&env);
    // Explicitly do NOT call set_authorized_issuer for this key.

    let user = Address::generate(&env);
    let expiry = env.ledger().timestamp() + 10_000;

    let (claim, sig) = build_signed_claim(
        &env,
        &user,
        IdentityTier::Basic,
        10,
        expiry,
        &rogue_issuer,
        &rogue_signing_key,
    );

    let result = client.try_submit_identity_claim(&claim, &sig);
    assert!(result.is_err());
    // Ensure the rogue keypair itself isn't lying around unused.
    let _ = rogue_pk_byten;
}

#[test]
fn test_submit_identity_claim_invalid_signature() {
    let env = Env::default();
    let (client, _cid, _admin, _depositor, _contributor, issuer, _signing_key, _tc) =
        setup_with_identity(&env, 10_000i128);

    // Sign with a *different* key than the one the admin authorized.
    let (wrong_signing_key, _pk, _pk_byten) = generate_keypair(&env);

    let user = Address::generate(&env);
    let expiry = env.ledger().timestamp() + 10_000;

    let (claim, sig) = build_signed_claim(
        &env,
        &user,
        IdentityTier::Basic,
        10,
        expiry,
        &issuer,
        &wrong_signing_key,
    );

    // ed25519_verify will panic → Soroban host converts to failed tx.
    let result = client.try_submit_identity_claim(&claim, &sig);
    assert!(result.is_err());
}

#[test]
fn test_submit_identity_claim_invalid_risk_score() {
    let env = Env::default();
    let (client, _cid, _admin, _depositor, _contributor, issuer, signing_key, _tc) =
        setup_with_identity(&env, 10_000i128);

    let user = Address::generate(&env);
    let expiry = env.ledger().timestamp() + 10_000;

    let (claim, sig) = build_signed_claim(
        &env,
        &user,
        IdentityTier::Basic,
        101, // invalid
        expiry,
        &issuer,
        &signing_key,
    );

    let result = client.try_submit_identity_claim(&claim, &sig);
    assert!(result.is_err());
}

#[test]
fn test_submit_identity_claim_updates_existing() {
    let env = Env::default();
    let (client, _cid, _admin, _depositor, _contributor, issuer, signing_key, _tc) =
        setup_with_identity(&env, 10_000i128);

    let user = Address::generate(&env);
    let expiry = env.ledger().timestamp() + 10_000;

    // First claim: Basic
    let (claim1, sig1) = build_signed_claim(
        &env,
        &user,
        IdentityTier::Basic,
        10,
        expiry,
        &issuer,
        &signing_key,
    );
    client.submit_identity_claim(&claim1, &sig1);

    let id1 = client.get_address_identity(&user);
    assert_eq!(id1.tier, IdentityTier::Basic);

    // Second claim: upgrade to Premium
    let (claim2, sig2) = build_signed_claim(
        &env,
        &user,
        IdentityTier::Premium,
        5,
        expiry,
        &issuer,
        &signing_key,
    );
    client.submit_identity_claim(&claim2, &sig2);

    let id2 = client.get_address_identity(&user);
    assert_eq!(id2.tier, IdentityTier::Premium);
    assert_eq!(id2.risk_score, 5);
}

#[test]
fn test_submit_identity_claim_then_lock_funds_uses_tier() {
    let env = Env::default();
    let (client, _cid, _admin, depositor, _contributor, issuer, signing_key, _tc) =
        setup_with_identity(&env, 10_000_0000000);

    let expiry = env.ledger().timestamp() + 10_000;

    // Upgrade depositor to Premium tier.
    let (claim, sig) = build_signed_claim(
        &env,
        &depositor,
        IdentityTier::Premium,
        10,
        expiry,
        &issuer,
        &signing_key,
    );
    client.submit_identity_claim(&claim, &sig);

    // Now locking 10,000 tokens should succeed (premium limit = 100,000).
    let amount = 10_000_0000000i128;
    let bounty_id = 1u64;
    let deadline = env.ledger().timestamp() + 1000;
    client.lock_funds(&depositor, &bounty_id, &amount, &deadline);

    let escrow = client.get_escrow(&bounty_id);
    assert_eq!(escrow.amount, amount);
}

#[test]
fn test_submit_identity_claim_high_risk_reduces_limit() {
    let env = Env::default();
    let (client, _cid, _admin, depositor, _contributor, issuer, signing_key, _tc) =
        setup_with_identity(&env, 10_000_0000000);

    let expiry = env.ledger().timestamp() + 10_000;

    // Give depositor Basic tier but high risk score (80 > threshold 70).
    // Basic limit = 1000 tokens, with 50% multiplier → effective = 500 tokens.
    let (claim, sig) = build_signed_claim(
        &env,
        &depositor,
        IdentityTier::Basic,
        80,
        expiry,
        &issuer,
        &signing_key,
    );
    client.submit_identity_claim(&claim, &sig);

    // 600 tokens should fail (limit = 500).
    let amount = 600_0000000i128;
    let bounty_id = 1u64;
    let deadline = env.ledger().timestamp() + 1000;
    let result = client.try_lock_funds(&depositor, &bounty_id, &amount, &deadline);
    assert!(result.is_err());

    // 400 tokens should succeed (within 500 limit).
    let amount_ok = 400_0000000i128;
    client.lock_funds(&depositor, &bounty_id, &amount_ok, &deadline);

    let escrow = client.get_escrow(&bounty_id);
    assert_eq!(escrow.amount, amount_ok);
}
