use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup_contract(env: &Env) -> (BountyEscrowContractClient<'static>, Address) {
    env.mock_all_auths();

    let contract_id = env.register_contract(None, BountyEscrowContract);
    let client = BountyEscrowContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    let token_admin = Address::generate(env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    client.init(&admin, &token_id);

    (client, admin)
}

#[test]
fn test_escrow_risk_flags_set_clear_and_persist() {
    let env = Env::default();
    let (client, _admin) = setup_contract(&env);

    let bounty_id = 42u64;
    let flags = RISK_FLAG_HIGH_RISK | RISK_FLAG_UNDER_REVIEW;

    let updated = client.set_escrow_risk_flags(&bounty_id, &flags);
    assert_eq!(updated.risk_flags, flags);

    let cleared = client.clear_escrow_risk_flags(&bounty_id, &RISK_FLAG_UNDER_REVIEW);
    assert_eq!(cleared.risk_flags, RISK_FLAG_HIGH_RISK);

    client.update_metadata(
        &_admin,
        &bounty_id,
        &123,
        &456,
        &soroban_sdk::String::from_str(&env, "bug_fix"),
        &None,
    );

    let fetched = client.get_metadata(&bounty_id);
    assert_eq!(fetched.risk_flags, RISK_FLAG_HIGH_RISK);
}
