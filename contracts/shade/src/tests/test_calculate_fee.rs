#![cfg(test)]

use crate::components::admin as admin_component;
use crate::shade::Shade;
use crate::shade::ShadeClient;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, BytesN, Env};

fn setup(env: &Env) -> (Address, ShadeClient<'_>, Address) {
    env.mock_all_auths();

    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(env, &contract_id);

    let admin = Address::generate(env);
    let wasm_hash = BytesN::from_array(&env, &[0; 32]);
    client.initialize(&admin, &wasm_hash);

    let token_admin = Address::generate(env);
    let token = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();

    client.add_accepted_token(&admin, &token);

    (admin, client, token)
}

// No fee set → always returns 0 regardless of amount.
#[test]
fn test_calculate_fee_returns_zero_when_no_fee_set() {
    let env = Env::default();
    let (_admin, client, token) = setup(&env);
    let merchant = Address::generate(&env);
    let contract_id = client.address.clone();
    env.as_contract(&contract_id, || {
        assert_eq!(
            admin_component::calculate_fee(&env, &merchant, &token, 0),
            0
        );
        assert_eq!(
            admin_component::calculate_fee(&env, &merchant, &token, 1_000),
            0
        );
        assert_eq!(
            admin_component::calculate_fee(&env, &merchant, &token, 1_000_000),
            0
        );
    });
}

// Zero amount with any fee → always returns 0.
#[test]
fn test_calculate_fee_zero_amount() {
    let env = Env::default();
    let (admin, client, token) = setup(&env);
    let contract_id = client.address.clone();
    let merchant = Address::generate(&env);

    client.set_fee(&admin, &token, &500); // 5%

    env.as_contract(&contract_id, || {
        assert_eq!(
            admin_component::calculate_fee(&env, &merchant, &token, 0),
            0
        );
    });
}

// 500 bps = 5%: fee on 1_000 should be 50.
#[test]
fn test_calculate_fee_5_percent() {
    let env = Env::default();
    let (admin, client, token) = setup(&env);
    let contract_id = client.address.clone();
    let merchant = Address::generate(&env);

    client.set_fee(&admin, &token, &500);

    env.as_contract(&contract_id, || {
        assert_eq!(
            admin_component::calculate_fee(&env, &merchant, &token, 1_000),
            50
        );
    });
}

// 100 bps = 1%: fee on 10_000 should be 100.
#[test]
fn test_calculate_fee_1_percent() {
    let env = Env::default();
    let (admin, client, token) = setup(&env);
    let contract_id = client.address.clone();
    let merchant = Address::generate(&env);

    client.set_fee(&admin, &token, &100);

    env.as_contract(&contract_id, || {
        assert_eq!(
            admin_component::calculate_fee(&env, &merchant, &token, 10_000),
            100
        );
    });
}

// 1_000 bps = 10%: fee on 5_000 should be 500.
#[test]
fn test_calculate_fee_10_percent() {
    let env = Env::default();
    let (admin, client, token) = setup(&env);
    let contract_id = client.address.clone();
    let merchant = Address::generate(&env);

    client.set_fee(&admin, &token, &1_000);

    env.as_contract(&contract_id, || {
        assert_eq!(
            admin_component::calculate_fee(&env, &merchant, &token, 5_000),
            500
        );
    });
}

// 10_000 bps = 100%: fee equals the full amount.
#[test]
fn test_calculate_fee_100_percent() {
    let env = Env::default();
    let (admin, client, token) = setup(&env);
    let contract_id = client.address.clone();
    let merchant = Address::generate(&env);

    client.set_fee(&admin, &token, &10_000);

    env.as_contract(&contract_id, || {
        assert_eq!(
            admin_component::calculate_fee(&env, &merchant, &token, 1_000),
            1_000
        );
    });
}

// 1 bps on an amount that produces a sub-unit result truncates toward zero.
// 1 bps on 1 → (1 * 1) / 10_000 = 0 (integer truncation).
#[test]
fn test_calculate_fee_truncates_fractional_result() {
    let env = Env::default();
    let (admin, client, token) = setup(&env);
    let contract_id = client.address.clone();
    let merchant = Address::generate(&env);

    client.set_fee(&admin, &token, &1); // 0.01%

    env.as_contract(&contract_id, || {
        // Too small to produce a whole unit
        assert_eq!(
            admin_component::calculate_fee(&env, &merchant, &token, 1),
            0
        );
        assert_eq!(
            admin_component::calculate_fee(&env, &merchant, &token, 9_999),
            0
        );
        // Exactly at the boundary: 10_000 * 1 / 10_000 = 1
        assert_eq!(
            admin_component::calculate_fee(&env, &merchant, &token, 10_000),
            1
        );
    });
}

// Updating the fee changes the computed amount accordingly.
#[test]
fn test_calculate_fee_reflects_updated_fee() {
    let env = Env::default();
    let (admin, client, token) = setup(&env);
    let contract_id = client.address.clone();
    let merchant = Address::generate(&env);

    client.set_fee(&admin, &token, &200); // 2%

    env.as_contract(&contract_id, || {
        assert_eq!(
            admin_component::calculate_fee(&env, &merchant, &token, 10_000),
            200
        );
    });

    client.set_fee(&admin, &token, &500); // updated to 5%

    env.as_contract(&contract_id, || {
        assert_eq!(
            admin_component::calculate_fee(&env, &merchant, &token, 10_000),
            500
        );
    });
}

// Large amounts stay within i128 range and produce correct results.
// 250 bps (2.5%) on 1_000_000_000 should be 25_000_000.
#[test]
fn test_calculate_fee_large_amount() {
    let env = Env::default();
    let (admin, client, token) = setup(&env);
    let contract_id = client.address.clone();
    let merchant = Address::generate(&env);

    client.set_fee(&admin, &token, &250); // 2.5%

    env.as_contract(&contract_id, || {
        assert_eq!(
            admin_component::calculate_fee(&env, &merchant, &token, 1_000_000_000),
            25_000_000
        );
    });
}

// Each token tracks its own fee independently.
#[test]
fn test_calculate_fee_per_token_independence() {
    let env = Env::default();
    let (admin, client, token_a) = setup(&env);
    let contract_id = client.address.clone();
    let merchant = Address::generate(&env);

    // Register a second token
    let token_b_admin = Address::generate(&env);
    let token_b = env
        .register_stellar_asset_contract_v2(token_b_admin)
        .address();
    client.add_accepted_token(&admin, &token_b);

    client.set_fee(&admin, &token_a, &300); // 3%
    client.set_fee(&admin, &token_b, &700); // 7%

    env.as_contract(&contract_id, || {
        assert_eq!(
            admin_component::calculate_fee(&env, &merchant, &token_a, 10_000),
            300
        );
        assert_eq!(
            admin_component::calculate_fee(&env, &merchant, &token_b, 10_000),
            700
        );
    });
}
