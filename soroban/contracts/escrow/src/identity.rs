//! Identity-aware limits module for escrow contract.
//!
//! This module provides address binding rules and identity verification for the Soroban escrow contract.
//! It handles off-chain identity claims, signature verification, and tier-based limits to prevent
//! spoofed identities on claims.
//!
//! ## Security Model
//!
//! - **Address Binding**: Claims are bound to specific addresses via Ed25519 signature verification
//! - **Issuer Authorization**: Only authorized issuers can issue identity claims
//! - **Time-based Expiry**: Claims expire after a specified timestamp to limit replay attack window
//! - **Tier-based Limits**: Different tiers have different transaction limits
//! - **Risk Adjustments**: High-risk addresses have reduced limits
//!
//! ## Usage
//!
//! 1. Admin authorizes an issuer via `set_authorized_issuer`
//! 2. Users submit identity claims signed by the authorized issuer
//! 3. The contract verifies the signature and stores the identity
//! 4. Transaction limits are enforced based on the user's tier and risk score

use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env};

use crate::Error;

/// Identity tier levels for KYC verification.
///
/// Higher tiers have higher transaction limits. The tier is set by authorized
/// issuers based on KYC verification level.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum IdentityTier {
    /// Unverified tier - default for addresses without a valid claim.
    /// Limited to basic transaction amounts (100 tokens default).
    Unverified = 0,
    /// Basic tier - passed basic KYC verification.
    /// Higher limits than unverified (1,000 tokens default).
    Basic = 1,
    /// Verified tier - passed full KYC verification.
    /// Significant limits (10,000 tokens default).
    Verified = 2,
    /// Premium tier - trusted users with highest limits.
    /// Maximum limits (100,000 tokens default).
    Premium = 3,
}

/// Identity claim structure signed by authorized issuers.
///
/// This struct represents an identity claim that is signed by an authorized issuer.
/// The claim binds an address to a specific tier and risk score, enabling the contract
/// to enforce appropriate transaction limits.
///
/// # Security
///
/// The claim includes the address to prevent signature reuse across different addresses.
/// The signature is verified using Ed25519, ensuring only authorized issuers can create claims.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityClaim {
    /// The address this claim is bound to.
    /// Cannot be used with any other address.
    pub address: Address,
    /// The identity tier level (0-3).
    pub tier: IdentityTier,
    /// Risk score (0-100). Higher scores indicate higher risk.
    pub risk_score: u32,
    /// Unix timestamp when this claim expires.
    /// After expiry, the claim is no longer valid.
    pub expiry: u64,
    /// The issuer's public key (Address).
    /// Must be an authorized issuer in the contract.
    pub issuer: Address,
}

/// Stored identity data for an address.
///
/// This struct is stored persistently and represents the verified identity
/// of an address on-chain.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddressIdentity {
    /// The identity tier level.
    pub tier: IdentityTier,
    /// Risk score (0-100).
    pub risk_score: u32,
    /// Unix timestamp when this identity expires.
    pub expiry: u64,
    /// Unix timestamp of the last update.
    pub last_updated: u64,
}

/// Configuration for tier-based transaction limits.
///
/// Defines the maximum transaction amount for each identity tier.
/// Values are in stroops (7 decimal places).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TierLimits {
    /// Maximum amount for unverified addresses.
    pub unverified_limit: i128,
    /// Maximum amount for basic tier addresses.
    pub basic_limit: i128,
    /// Maximum amount for verified tier addresses.
    pub verified_limit: i128,
    /// Maximum amount for premium tier addresses.
    pub premium_limit: i128,
}

/// Configuration for risk-based limit adjustments.
///
/// When an address has a risk score above the threshold,
/// their limit is reduced by the multiplier percentage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskThresholds {
    /// Risk score threshold above which limits are reduced.
    pub high_risk_threshold: u32,
    /// Percentage of tier limit allowed when risk is high (0-100).
    pub high_risk_multiplier: u32,
}

impl Default for AddressIdentity {
    /// Creates a default address identity with unverified tier and zero risk score.
    fn default() -> Self {
        Self {
            tier: IdentityTier::Unverified,
            risk_score: 0,
            expiry: 0,
            last_updated: 0,
        }
    }
}

impl Default for TierLimits {
    /// Creates default tier limits:
    /// - Unverified: 100 tokens
    /// - Basic: 1,000 tokens
    /// - Verified: 10,000 tokens
    /// - Premium: 100,000 tokens
    fn default() -> Self {
        Self {
            unverified_limit: 100_0000000, // 100 tokens (7 decimals)
            basic_limit: 1000_0000000,     // 1,000 tokens
            verified_limit: 10000_0000000, // 10,000 tokens
            premium_limit: 100000_0000000, // 100,000 tokens
        }
    }
}

impl Default for RiskThresholds {
    /// Creates default risk thresholds:
    /// - High risk threshold: 70
    /// - High risk multiplier: 50% (half of tier limit)
    fn default() -> Self {
        Self {
            high_risk_threshold: 70,
            high_risk_multiplier: 50, // 50% of tier limit
        }
    }
}

/// Serialize an identity claim for signature verification.
///
/// Uses deterministic XDR encoding to ensure consistent signatures.
/// The serialization format includes all claim fields in a fixed order.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `claim` - The identity claim to serialize
///
/// # Returns
/// A `Bytes` containing the serialized claim data
pub fn serialize_claim(env: &Env, claim: &IdentityClaim) -> Bytes {
    // Serialize claim to bytes using Soroban's serialization
    // This creates a deterministic byte representation
    let mut bytes = Bytes::new(env);

    // Serialize each field in order
    bytes.append(&claim.address.clone().to_xdr(env));
    bytes.append(&Bytes::from_array(
        env,
        &[
            (claim.tier.clone() as u32).to_be_bytes()[0],
            (claim.tier.clone() as u32).to_be_bytes()[1],
            (claim.tier.clone() as u32).to_be_bytes()[2],
            (claim.tier.clone() as u32).to_be_bytes()[3],
        ],
    ));
    bytes.append(&Bytes::from_array(env, &claim.risk_score.to_be_bytes()));
    bytes.append(&Bytes::from_array(env, &claim.expiry.to_be_bytes()));
    bytes.append(&claim.issuer.clone().to_xdr(env));

    bytes
}

/// Verify the signature of an identity claim.
///
/// Uses Ed25519 signature verification to ensure the claim was signed by
/// the authorized issuer's private key.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `claim` - The identity claim to verify
/// * `signature` - The Ed25519 signature (64 bytes)
/// * `issuer_pubkey` - The issuer's public key (32 bytes), looked up from
///   the on-chain authorization store
///
/// # Panics
/// Panics if the signature is invalid.  The Soroban host converts the panic
/// into a failed transaction, so callers always observe an error on a bad
/// signature — just not the contract-defined `Error::InvalidSignature` code.
pub fn verify_claim_signature(
    env: &Env,
    claim: &IdentityClaim,
    signature: &BytesN<64>,
    issuer_pubkey: &BytesN<32>,
) {
    let message = serialize_claim(env, claim);
    env.crypto()
        .ed25519_verify(issuer_pubkey, &message, signature);
}

/// Check if a claim has expired based on the current ledger timestamp.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `expiry` - Unix timestamp to check against current time
///
/// # Returns
/// * `true` if the current time is >= expiry timestamp
/// * `false` if the claim is still valid
pub fn is_claim_expired(env: &Env, expiry: u64) -> bool {
    let now = env.ledger().timestamp();
    now >= expiry
}

/// Validate claim format and fields.
///
/// Checks that all claim fields are within acceptable ranges.
///
/// # Arguments
/// * `claim` - The identity claim to validate
///
/// # Returns
/// * `Ok(())` if claim is valid
/// * `Err(Error::InvalidRiskScore)` if risk score is out of range (0-100)
/// * `Err(Error::InvalidTier)` if tier discriminant is unknown (> 3)
pub fn validate_claim(claim: &IdentityClaim) -> Result<(), Error> {
    // Validate risk score is in valid range (0-100)
    if claim.risk_score > 100 {
        return Err(Error::InvalidRiskScore);
    }

    // Validate tier is a known variant (0-3)
    let tier_u32 = claim.tier.clone() as u32;
    if tier_u32 > 3 {
        return Err(Error::InvalidTier);
    }

    Ok(())
}

/// Calculate effective transaction limit based on tier and risk score.
///
/// This function applies both tier-based limits and risk-based adjustments.
/// High-risk addresses have their limits reduced by the risk multiplier.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `identity` - The stored address identity
/// * `tier_limits` - The tier limits configuration
/// * `risk_thresholds` - The risk thresholds configuration
///
/// # Returns
/// The effective transaction limit in stroops
pub fn calculate_effective_limit(
    _env: &Env,
    identity: &AddressIdentity,
    tier_limits: &TierLimits,
    risk_thresholds: &RiskThresholds,
) -> i128 {
    // Get tier-based limit
    let tier_limit = match identity.tier {
        IdentityTier::Unverified => tier_limits.unverified_limit,
        IdentityTier::Basic => tier_limits.basic_limit,
        IdentityTier::Verified => tier_limits.verified_limit,
        IdentityTier::Premium => tier_limits.premium_limit,
    };

    // Apply risk-based adjustment if risk score is high
    if identity.risk_score >= risk_thresholds.high_risk_threshold {
        // Reduce limit by risk multiplier percentage
        let multiplier = risk_thresholds.high_risk_multiplier as i128;
        let risk_adjusted_limit = (tier_limit * multiplier) / 100;
        risk_adjusted_limit
    } else {
        tier_limit
    }
}
