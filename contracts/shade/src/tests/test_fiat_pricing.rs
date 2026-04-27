#![cfg(test)]

use crate::shade::{Shade, ShadeClient};
use crate::types::{OracleConfig, InvoicePricingMode};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{contract, contractimpl, Address, Env, String};

#[contract]
pub struct MockOracle;

#[contractimpl]
impl MockOracle {
    pub fn get_price(env: Env, _token: Address, _quote_currency: String) -> i128 {
        env.storage().instance().get(&"price").unwrap_or(100_000_000) // Default $1.00 if decimals=8
    }

    pub fn set_price(env: Env, price: i128) {
        env.storage().instance().set(&"price", &price);
    }
}

fn setup_test() -> (Env, ShadeClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, contract_id, admin)
}

fn create_test_token(env: &Env) -> Address {
    let token_admin = Address::generate(env);
    env.register_stellar_asset_contract_v2(token_admin)
        .address()
}

#[test]
fn test_fiat_invoice_creation_and_resolution() {
    let (env, client, _shade_id, admin) = setup_test();
    let token = create_test_token(&env);
    client.add_accepted_token(&admin, &token);

    let oracle_id = env.register(MockOracle, ());
    let oracle_client = MockOracleClient::new(&env, &oracle_id);
    
    // Set price to $2.00 (200,000,000 with 8 decimals)
    let initial_price = 200_000_000;
    oracle_client.set_price(&initial_price);

    let oracle_config = OracleConfig {
        contract: oracle_id.clone(),
        price_decimals: 8,
        token_decimals: 7, // 7 decimals for the token
    };
    client.set_token_oracle(&admin, &token, &oracle_config);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    // Create fiat invoice for $10.00 USD (1,000 with 2 decimals)
    let fiat_amount = 1000;
    let fiat_decimals = 2;
    let currency = String::from_str(&env, "USD");

    let invoice_id = client.create_fiat_invoice(
        &merchant,
        &String::from_str(&env, "Fiat test"),
        &fiat_amount,
        &currency,
        &fiat_decimals,
        &token,
        &None,
    );

    let invoice = client.get_invoice(&invoice_id);
    assert_eq!(invoice.pricing_mode, InvoicePricingMode::FixedFiat);
    
    // Expected crypto amount:
    // (fiat_amount * 10^token_decimals * 10^price_decimals) / (price * 10^fiat_decimals)
    // (1000 * 10^7 * 10^8) / (200,000,000 * 10^2)
    // (10^3 * 10^7 * 10^8) / (2 * 10^8 * 10^2)
    // 10^18 / (2 * 10^10) = 0.5 * 10^8 = 5 * 10^7
    let expected_amount = 50_000_000;
    assert_eq!(invoice.amount, expected_amount);

    // Verify resolve_invoice_amount
    assert_eq!(client.resolve_invoice_amount(&invoice_id), expected_amount);

    // Update price to $5.00
    oracle_client.set_price(&500_000_000);
    
    // New expected amount: 10^18 / (5 * 10^10) = 0.2 * 10^8 = 2 * 10^7
    let new_expected_amount = 20_000_000;
    assert_eq!(client.resolve_invoice_amount(&invoice_id), new_expected_amount);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #18)")]
fn test_fiat_invoice_fails_without_oracle() {
    let (env, client, _shade_id, admin) = setup_test();
    let token = create_test_token(&env);
    client.add_accepted_token(&admin, &token);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    client.create_fiat_invoice(
        &merchant,
        &String::from_str(&env, "No oracle test"),
        &1000,
        &String::from_str(&env, "USD"),
        &2,
        &token,
        &None,
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #18)")]
fn test_fiat_invoice_fails_with_invalid_price() {
    let (env, client, _shade_id, admin) = setup_test();
    let token = create_test_token(&env);
    client.add_accepted_token(&admin, &token);

    let oracle_id = env.register(MockOracle, ());
    let oracle_client = MockOracleClient::new(&env, &oracle_id);
    oracle_client.set_price(&0); // Invalid price

    let oracle_config = OracleConfig {
        contract: oracle_id,
        price_decimals: 8,
        token_decimals: 7,
    };
    client.set_token_oracle(&admin, &token, &oracle_config);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    client.create_fiat_invoice(
        &merchant,
        &String::from_str(&env, "Invalid price test"),
        &1000,
        &String::from_str(&env, "USD"),
        &2,
        &token,
        &None,
    );
}
