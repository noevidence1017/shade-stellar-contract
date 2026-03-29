#![cfg(test)]

use crate::shade::Shade;
use crate::shade::ShadeClient;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, BytesN, Env, String};

fn setup(env: &Env) -> (Address, ShadeClient<'_>, Address, Address, Address) {
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
    client.set_fee(&admin, &token, &1000); // 10% base fee in bps

    let merchant = Address::generate(env);
    client.register_merchant(&merchant);
    client.verify_merchant(&admin, &1, &true);

    // Set a distinct merchant account address (a plain wallet), same pattern as test_payment tests.
    let merchant_account = Address::generate(env);
    client.set_merchant_account(&merchant, &merchant_account);

    (admin, client, token, merchant, merchant_account)
}

#[test]
fn test_fee_discounts_based_on_volume() {
    let env = Env::default();
    let (_admin, client, token, merchant, merchant_account) = setup(&env);
    let payer = Address::generate(&env);

    // Fund the payer
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    token_client.mint(&payer, &1_000_000);

    let token_balance_client = soroban_sdk::token::TokenClient::new(&env, &token);

    // Volume 0 -> Fee 10% (100). Merchant receives 900. Volume becomes 1,000.
    let inv1 = client.create_invoice(
        &merchant,
        &String::from_str(&env, "inv1"),
        &1000,
        &token,
        &None,
    );
    client.pay_invoice(&payer, &inv1);

    // Volume 1,000 -> Tier 0. Fee 10% (900). Merchant receives 8,100. Volume becomes 10,000.
    let inv2 = client.create_invoice(
        &merchant,
        &String::from_str(&env, "inv2"),
        &9000,
        &token,
        &None,
    );
    client.pay_invoice(&payer, &inv2);

    // Volume 10,000 -> Tier 1. Discount 10% -> Fee 9% (90). Merchant receives 910. Volume becomes 11,000.
    let inv3 = client.create_invoice(
        &merchant,
        &String::from_str(&env, "inv3"),
        &1000,
        &token,
        &None,
    );
    client.pay_invoice(&payer, &inv3);

    // 900 + 8100 + 910 = 9910.
    assert_eq!(token_balance_client.balance(&merchant_account), 9910);

    // Volume 11,000 -> Tier 1. Fee 9% (3510). Merchant receives 35,490. Volume becomes 50,000.
    let inv4 = client.create_invoice(
        &merchant,
        &String::from_str(&env, "inv4"),
        &39000,
        &token,
        &None,
    );
    client.pay_invoice(&payer, &inv4);

    // Volume 50,000 -> Tier 2. Discount 25% -> Fee 7.5% (75). Merchant receives 925. Volume becomes 51,000.
    let inv5 = client.create_invoice(
        &merchant,
        &String::from_str(&env, "inv5"),
        &1000,
        &token,
        &None,
    );
    client.pay_invoice(&payer, &inv5);

    // 9910 + 35490 + 925 = 46325.
    assert_eq!(token_balance_client.balance(&merchant_account), 46325);
}

#[test]
fn test_volume_tracking_tier_3() {
    let env = Env::default();
    let (_admin, client, token, merchant, merchant_account) = setup(&env);
    let payer = Address::generate(&env);

    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    token_client.mint(&payer, &1_000_000);

    let token_balance_client = soroban_sdk::token::TokenClient::new(&env, &token);

    // Volume 0 -> Tier 0. Fee 10% (20,000). Merchant receives 180,000. Volume becomes 200,000.
    let inv1 = client.create_invoice(
        &merchant,
        &String::from_str(&env, "inv_tier3"),
        &200000,
        &token,
        &None,
    );
    client.pay_invoice(&payer, &inv1);

    // Volume 200,000 -> Tier 3. Discount 50% -> Fee 5% (50). Merchant receives 950.
    let inv2 = client.create_invoice(
        &merchant,
        &String::from_str(&env, "inv_tier3_next"),
        &1000,
        &token,
        &None,
    );
    client.pay_invoice(&payer, &inv2);

    // 180,000 + 950 = 180,950.
    assert_eq!(token_balance_client.balance(&merchant_account), 180950);
}
