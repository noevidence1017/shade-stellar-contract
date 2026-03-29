#![cfg(test)]

use crate::errors::ContractError;
use crate::shade::Shade;
use crate::shade::ShadeClient;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, BytesN, Env};

fn setup(env: &Env) -> (Address, ShadeClient, Address) {
    env.mock_all_auths();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(env, &contract_id);

    let admin = Address::generate(env);
    let wasm_hash = BytesN::from_array(env, &[0; 32]);
    client.initialize(&admin, &wasm_hash);

    let token_admin = Address::generate(env);
    let token = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    client.add_accepted_token(&admin, &token);

    (admin, client, token)
}

#[test]
fn test_time_locked_fee_update_success() {
    let env = Env::default();
    let (admin, client, token) = setup(&env);

    let new_fee = 500; // 5%
    client.propose_fee(&admin, &token, &new_fee);

    // Should still be old fee (default 0)
    assert_eq!(client.get_fee(&token), 0);

    // Fast forward 49 hours (delay is 48)
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 49 * 3600);

    client.execute_fee(&admin, &token);
    assert_eq!(client.get_fee(&token), new_fee);
}

#[test]
fn test_execute_fee_too_early() {
    let env = Env::default();
    let (admin, client, token) = setup(&env);

    client.propose_fee(&admin, &token, &500);

    // Fast forward only 10 hours
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 10 * 3600);

    let result = client.try_execute_fee(&admin, &token);
    let expected_error =
        soroban_sdk::Error::from_contract_error(ContractError::FeeUpdateTooEarly as u32);
    assert!(matches!(result, Err(Ok(err)) if err == expected_error));
}

#[test]
fn test_execute_fee_no_pending() {
    let env = Env::default();
    let (admin, client, token) = setup(&env);

    let result = client.try_execute_fee(&admin, &token);
    let expected_error =
        soroban_sdk::Error::from_contract_error(ContractError::NoPendingFeeUpdate as u32);
    assert!(matches!(result, Err(Ok(err)) if err == expected_error));
}
