#![cfg(test)]

use crate::errors::ContractError;
use crate::shade::{Shade, ShadeClient};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{token, Address, Env, String};

fn setup_test() -> (Env, ShadeClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, contract_id, admin)
}

fn create_token(env: &Env) -> Address {
    env.register_stellar_asset_contract_v2(Address::generate(env))
        .address()
}

// ---------------------------------------------------------------------------
// pay_invoice panics with InvoiceExpired after ledger time >= expires_at
// ---------------------------------------------------------------------------

#[test]
fn test_pay_invoice_panics_when_expired() {
    let (env, client, _contract_id, admin) = setup_test();

    let token = create_token(&env);
    client.add_accepted_token(&admin, &token);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let merchant_account = Address::generate(&env);
    client.set_merchant_account(&merchant, &merchant_account);

    // create invoice that expires at t=1000
    env.ledger().set_timestamp(500);
    let invoice_id = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Expiring Invoice"),
        &1000,
        &token,
        &Some(1000u64),
    );

    // mint tokens for payer
    let payer = Address::generate(&env);
    let token_admin = token::StellarAssetClient::new(&env, &token);
    token_admin.mint(&payer, &1000);

    // advance ledger past expiry
    env.ledger().set_timestamp(1000);

    let expected_error =
        soroban_sdk::Error::from_contract_error(ContractError::InvoiceExpired as u32);
    let result = client.try_pay_invoice(&payer, &invoice_id);
    assert!(matches!(result, Err(Ok(err)) if err == expected_error));
}

#[test]
fn test_pay_invoice_panics_well_past_expiry() {
    let (env, client, _contract_id, admin) = setup_test();

    let token = create_token(&env);
    client.add_accepted_token(&admin, &token);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let merchant_account = Address::generate(&env);
    client.set_merchant_account(&merchant, &merchant_account);

    env.ledger().set_timestamp(100);
    let invoice_id = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Long Expired Invoice"),
        &500,
        &token,
        &Some(200u64),
    );

    let payer = Address::generate(&env);
    let token_admin = token::StellarAssetClient::new(&env, &token);
    token_admin.mint(&payer, &500);

    // advance far past expiry
    env.ledger().set_timestamp(9999);

    let expected_error =
        soroban_sdk::Error::from_contract_error(ContractError::InvoiceExpired as u32);
    let result = client.try_pay_invoice(&payer, &invoice_id);
    assert!(matches!(result, Err(Ok(err)) if err == expected_error));
}

// ---------------------------------------------------------------------------
// pay_invoice succeeds when ledger time is strictly before expires_at
// ---------------------------------------------------------------------------

#[test]
fn test_pay_invoice_succeeds_before_expiry() {
    let (env, client, _contract_id, admin) = setup_test();

    let token = create_token(&env);
    client.add_accepted_token(&admin, &token);
    client.set_fee(&admin, &token, &0);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let merchant_account = Address::generate(&env);
    client.set_merchant_account(&merchant, &merchant_account);

    env.ledger().set_timestamp(100);
    let invoice_id = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Valid Invoice"),
        &1000,
        &token,
        &Some(2000u64),
    );

    let payer = Address::generate(&env);
    let token_admin = token::StellarAssetClient::new(&env, &token);
    token_admin.mint(&payer, &1000);

    // still before expiry
    env.ledger().set_timestamp(999);

    client.pay_invoice(&payer, &invoice_id);

    let invoice = client.get_invoice(&invoice_id);
    assert_eq!(invoice.status, crate::types::InvoiceStatus::Paid);
}

// ---------------------------------------------------------------------------
// invoice with no expiry is always payable regardless of ledger time
// ---------------------------------------------------------------------------

#[test]
fn test_pay_invoice_no_expiry_always_valid() {
    let (env, client, _contract_id, admin) = setup_test();

    let token = create_token(&env);
    client.add_accepted_token(&admin, &token);
    client.set_fee(&admin, &token, &0);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let merchant_account = Address::generate(&env);
    client.set_merchant_account(&merchant, &merchant_account);

    env.ledger().set_timestamp(1);
    let invoice_id = client.create_invoice(
        &merchant,
        &String::from_str(&env, "No Expiry Invoice"),
        &1000,
        &token,
        &None,
    );

    let payer = Address::generate(&env);
    let token_admin = token::StellarAssetClient::new(&env, &token);
    token_admin.mint(&payer, &1000);

    // advance to a very large timestamp — no expiry set, should still succeed
    env.ledger().set_timestamp(u64::MAX / 2);

    client.pay_invoice(&payer, &invoice_id);

    let invoice = client.get_invoice(&invoice_id);
    assert_eq!(invoice.status, crate::types::InvoiceStatus::Paid);
}

// ---------------------------------------------------------------------------
// create_invoice itself panics when expires_at is already in the past
// ---------------------------------------------------------------------------

#[test]
fn test_create_invoice_panics_when_expires_at_in_past() {
    let (env, client, _contract_id, admin) = setup_test();

    let token = create_token(&env);
    client.add_accepted_token(&admin, &token);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    env.ledger().set_timestamp(5000);

    // expires_at = 1000 is in the past relative to ledger timestamp 5000
    let expected_error =
        soroban_sdk::Error::from_contract_error(ContractError::InvoiceExpired as u32);
    let result = client.try_create_invoice(
        &merchant,
        &String::from_str(&env, "Already Expired"),
        &100,
        &token,
        &Some(1000u64),
    );
    assert!(matches!(result, Err(Ok(err)) if err == expected_error));
}
