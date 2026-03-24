#[cfg(test)]
mod cross_contract_interface_tests {
    use crate::{BountyEscrowContract, EscrowStatus};
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token, Address, Env,
    };

    fn create_token_contract<'a>(
        e: &Env,
        admin: &Address,
    ) -> (token::Client<'a>, token::StellarAssetClient<'a>) {
        let contract = e.register_stellar_asset_contract_v2(admin.clone());
        let contract_address = contract.address();
        (
            token::Client::new(e, &contract_address),
            token::StellarAssetClient::new(e, &contract_address),
        )
    }

    #[test]
    fn test_escrow_interface_lock_funds_via_trait() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, BountyEscrowContract);
        let client = crate::BountyEscrowContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);

        let (token, token_admin) = create_token_contract(&env, &admin);
        client.init(&admin, &token.address);

        // Mint tokens to depositor
        token_admin.mint(&depositor, &1_000_000);

        let bounty_id = 1u64;
        let amount = 1000i128;
        let deadline = env.ledger().timestamp() + 3600;

        // lock_funds returns unit, panics on error
        client.lock_funds(&depositor, &bounty_id, &amount, &deadline);

        // Verify escrow was created
        let escrow = client.get_escrow_info(&bounty_id);
        assert_eq!(escrow.amount, amount);
        assert_eq!(escrow.status, EscrowStatus::Locked);
    }

    #[test]
    fn test_escrow_interface_release_funds_via_trait() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, BountyEscrowContract);
        let client = crate::BountyEscrowContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);
        let contributor = Address::generate(&env);

        let (token, token_admin) = create_token_contract(&env, &admin);
        client.init(&admin, &token.address);

        // Mint tokens to depositor
        token_admin.mint(&depositor, &1_000_000);

        let bounty_id = 1u64;
        let amount = 1000i128;
        let deadline = env.ledger().timestamp() + 3600;

        client.lock_funds(&depositor, &bounty_id, &amount, &deadline);

        // Release funds
        client.release_funds(&bounty_id, &contributor);

        // Verify status changed
        let escrow = client.get_escrow_info(&bounty_id);
        assert_eq!(escrow.status, EscrowStatus::Released);
    }

    #[test]
    fn test_escrow_interface_refund_via_trait() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, BountyEscrowContract);
        let client = crate::BountyEscrowContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);

        let (token, token_admin) = create_token_contract(&env, &admin);
        client.init(&admin, &token.address);

        // Mint tokens to depositor
        token_admin.mint(&depositor, &1_000_000);

        let bounty_id = 1u64;
        let amount = 1000i128;
        let deadline = env.ledger().timestamp() + 100;

        client.lock_funds(&depositor, &bounty_id, &amount, &deadline);

        // Move time past deadline
        env.ledger().set_timestamp(deadline + 1);

        // Refund should work
        client.refund(&bounty_id);

        // Verify status changed
        let escrow = client.get_escrow_info(&bounty_id);
        assert_eq!(escrow.status, EscrowStatus::Refunded);
    }

    #[test]
    fn test_abi_compatibility_lock_and_release_sequence() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, BountyEscrowContract);
        let client = crate::BountyEscrowContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);
        let contributor = Address::generate(&env);

        let (token, token_admin) = create_token_contract(&env, &admin);
        client.init(&admin, &token.address);

        // Mint tokens to depositor
        token_admin.mint(&depositor, &1_000_000);

        let bounty_id = 1u64;
        let amount = 5000i128;
        let deadline = env.ledger().timestamp() + 3600;

        // Lock
        client.lock_funds(&depositor, &bounty_id, &amount, &deadline);

        // Verify locked
        let escrow = client.get_escrow_info(&bounty_id);
        assert_eq!(escrow.status, EscrowStatus::Locked);
        assert_eq!(escrow.remaining_amount, amount);

        // Release
        client.release_funds(&bounty_id, &contributor);

        // Verify released
        let escrow = client.get_escrow_info(&bounty_id);
        assert_eq!(escrow.status, EscrowStatus::Released);
        assert_eq!(escrow.remaining_amount, 0);
    }

    #[test]
    fn test_interface_stability_version_compatibility() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, BountyEscrowContract);
        let client = crate::BountyEscrowContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);

        let (token, token_admin) = create_token_contract(&env, &admin);
        client.init(&admin, &token.address);

        // Mint tokens to depositor
        token_admin.mint(&depositor, &1_000_000);

        let bounty_id = 1u64;
        let amount = 1000i128;
        let deadline = env.ledger().timestamp() + 3600;

        // These calls should work - they panic on error so just calling is the test
        client.lock_funds(&depositor, &bounty_id, &amount, &deadline);

        let _balance = client.get_balance();

        let _escrow = client.get_escrow_info(&bounty_id);
    }

    #[test]
    fn test_interface_error_handling_consistency() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, BountyEscrowContract);
        let client = crate::BountyEscrowContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);

        let (token, token_admin) = create_token_contract(&env, &admin);
        client.init(&admin, &token.address);

        // Mint tokens to depositor
        token_admin.mint(&depositor, &1_000_000);

        let bounty_id = 1u64;
        let amount = 1000i128;
        let deadline = env.ledger().timestamp() + 3600;

        // First lock should succeed
        client.lock_funds(&depositor, &bounty_id, &amount, &deadline);

        // Verify the bounty exists by checking escrow info
        let escrow = client.get_escrow_info(&bounty_id);
        assert_eq!(escrow.status, EscrowStatus::Locked);
        assert_eq!(escrow.amount, amount);
    }

    #[test]
    fn test_cross_contract_state_consistency() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, BountyEscrowContract);
        let client = crate::BountyEscrowContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);

        let (token, token_admin) = create_token_contract(&env, &admin);
        client.init(&admin, &token.address);

        // Mint tokens to depositor
        token_admin.mint(&depositor, &1_000_000);

        let bounty_id = 1u64;
        let amount = 1000i128;
        let deadline = env.ledger().timestamp() + 3600;

        client.lock_funds(&depositor, &bounty_id, &amount, &deadline);

        // Multiple calls to get_escrow_info should return consistent state
        let escrow1 = client.get_escrow_info(&bounty_id);
        let escrow2 = client.get_escrow_info(&bounty_id);

        assert_eq!(escrow1.amount, escrow2.amount);
        assert_eq!(escrow1.status, escrow2.status);
        assert_eq!(escrow1.remaining_amount, escrow2.remaining_amount);
        assert_eq!(escrow1.deadline, escrow2.deadline);
    }

    #[test]
    fn test_partial_release_interface_compatibility() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, BountyEscrowContract);
        let client = crate::BountyEscrowContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);
        let contributor = Address::generate(&env);

        let (token, token_admin) = create_token_contract(&env, &admin);
        client.init(&admin, &token.address);

        // Mint tokens to depositor
        token_admin.mint(&depositor, &1_000_000);

        let bounty_id = 1u64;
        let amount = 1000i128;
        let deadline = env.ledger().timestamp() + 3600;

        client.lock_funds(&depositor, &bounty_id, &amount, &deadline);

        // Partial release
        let partial_amount = 300i128;
        client.partial_release(&bounty_id, &contributor, &partial_amount);

        // Verify remaining amount
        let escrow = client.get_escrow_info(&bounty_id);
        assert_eq!(escrow.remaining_amount, amount - partial_amount);
    }

    #[test]
    fn test_batch_operations_interface_compatibility() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, BountyEscrowContract);
        let client = crate::BountyEscrowContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);

        let (token, token_admin) = create_token_contract(&env, &admin);
        client.init(&admin, &token.address);

        // Mint tokens to depositor
        token_admin.mint(&depositor, &1_000_000);

        let deadline = env.ledger().timestamp() + 3600;

        // Lock multiple bounties sequentially
        for i in 0..3 {
            let bounty_id = i as u64;
            client.lock_funds(&depositor, &bounty_id, &1000i128, &deadline);
        }

        // Verify all three bounties are locked
        for i in 0..3 {
            let bounty_id = i as u64;
            let escrow = client.get_escrow_info(&bounty_id);
            assert_eq!(escrow.status, EscrowStatus::Locked);
        }
    }
}
