#![cfg(test)]

use crate::shade::{Shade, ShadeClient};
use crate::types::{DataKey, InvoiceStatus};
use account::account::{MerchantAccount, MerchantAccountClient};
use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _};
use soroban_sdk::{token, Address, Env, Map, String, Symbol, TryIntoVal, Val};

fn setup_test() -> (Env, ShadeClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, contract_id, admin)
}

fn assert_latest_invoice_event(
    env: &Env,
    contract_id: &Address,
    expected_invoice_id: u64,
    expected_merchant: &Address,
    expected_amount: i128,
    expected_token: &Address,
) {
    let events = env.events().all();
    assert!(!events.is_empty(), "No events captured for invoice!");

    let (event_contract_id, _topics, data) = events.get(events.len() - 1).unwrap();
    assert_eq!(&event_contract_id, contract_id);

    let data_map: Map<Symbol, Val> = data.try_into_val(env).unwrap();

    let invoice_id_val = data_map.get(Symbol::new(env, "invoice_id")).unwrap();
    let merchant_val = data_map.get(Symbol::new(env, "merchant")).unwrap();
    let amount_val = data_map.get(Symbol::new(env, "amount")).unwrap();
    let token_val = data_map.get(Symbol::new(env, "token")).unwrap();

    let invoice_id_in_event: u64 = invoice_id_val.try_into_val(env).unwrap();
    let merchant_in_event: Address = merchant_val.try_into_val(env).unwrap();
    let amount_in_event: i128 = amount_val.try_into_val(env).unwrap();
    let token_in_event: Address = token_val.try_into_val(env).unwrap();

    assert_eq!(invoice_id_in_event, expected_invoice_id);
    assert_eq!(merchant_in_event, expected_merchant.clone());
    assert_eq!(amount_in_event, expected_amount);
    assert_eq!(token_in_event, expected_token.clone());
}

fn create_test_token(env: &Env) -> Address {
    let token_admin = Address::generate(env);
    env.register_stellar_asset_contract_v2(token_admin)
        .address()
}

#[allow(clippy::too_many_arguments)]
fn mark_invoice_paid(
    env: &Env,
    shade_contract_id: &Address,
    merchant: &Address,
    invoice_id: u64,
    payer: &Address,
    date_paid: u64,
    merchant_account_id: &Address,
    client: &ShadeClient<'_>,
) {
    let mut invoice = client.get_invoice(&invoice_id);
    invoice.status = InvoiceStatus::Paid;
    invoice.payer = Some(payer.clone());
    invoice.date_paid = Some(date_paid);

    env.as_contract(shade_contract_id, || {
        env.storage()
            .persistent()
            .set(&DataKey::Invoice(invoice_id), &invoice);
        env.storage().persistent().set(
            &DataKey::MerchantBalance(merchant.clone()),
            merchant_account_id,
        );
    });
}

#[test]
fn test_create_and_get_invoice_success() {
    let (env, client, contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Test Invoice");
    let amount: i128 = 1000;

    let invoice_id = client.create_invoice(&merchant, &description, &amount, &token, &None);
    assert_eq!(invoice_id, 1);

    assert_latest_invoice_event(&env, &contract_id, invoice_id, &merchant, amount, &token);

    let invoice = client.get_invoice(&invoice_id);

    assert_eq!(invoice.id, 1);
    assert_eq!(invoice.merchant_id, 1);
    assert_eq!(invoice.amount, amount);
    assert_eq!(invoice.token, token);
    assert_eq!(invoice.description, description);
    assert_eq!(invoice.status, InvoiceStatus::Pending);
}

#[test]
fn test_create_multiple_invoices() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token1 = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token1);
    let token2 = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token2);

    let id1 = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Invoice 1"),
        &1000,
        &token1,
        &None,
    );
    let id2 = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Invoice 2"),
        &2000,
        &token2,
        &None,
    );
    let id3 = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Invoice 3"),
        &500,
        &token1,
        &None,
    );

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

#[should_panic(expected = "HostError: Error(Contract, #8)")]
#[test]
fn test_get_invoice_not_found() {
    let (_env, client, _contract_id, admin) = setup_test();
    client.get_invoice(&999);
}

#[should_panic(expected = "HostError: Error(Contract, #1)")]
#[test]
fn test_create_invoice_unregistered_merchant() {
    let (env, client, _contract_id, admin) = setup_test();

    let unregistered_merchant = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Test Invoice");
    let amount: i128 = 1000;

    client.create_invoice(&unregistered_merchant, &description, &amount, &token, &None);
}

#[should_panic(expected = "HostError: Error(Contract, #7)")]
#[test]
fn test_create_invoice_invalid_amount() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Test Invoice");
    let amount: i128 = 0;

    client.create_invoice(&merchant, &description, &amount, &token, &None);
}

#[test]
fn test_refund_invoice_success_within_window() {
    let (env, client, shade_contract_id, admin) = setup_test();
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = create_test_token(&env);
    client.add_accepted_token(&admin, &token);
    let payer = Address::generate(&env);
    let description = String::from_str(&env, "Refundable Invoice");
    let amount = 1_000_i128;
    let invoice_id = client.create_invoice(&merchant, &description, &amount, &token, &None);

    let merchant_account_id = env.register(MerchantAccount, ());
    let merchant_account = MerchantAccountClient::new(&env, &merchant_account_id);
    merchant_account.initialize(&merchant, &shade_contract_id, &1_u64);
    client.set_merchant_account(&merchant, &merchant_account_id);

    let token_admin = token::StellarAssetClient::new(&env, &token);
    token_admin.mint(&merchant_account_id, &amount);

    env.ledger().set_timestamp(1_000);
    mark_invoice_paid(
        &env,
        &shade_contract_id,
        &merchant,
        invoice_id,
        &payer,
        900,
        &merchant_account_id,
        &client,
    );

    client.refund_invoice(&merchant, &invoice_id);

    let updated = client.get_invoice(&invoice_id);
    assert_eq!(updated.status, InvoiceStatus::Refunded);
    assert_eq!(updated.amount_refunded, amount);

    let token_client = token::TokenClient::new(&env, &token);
    assert_eq!(token_client.balance(&payer), amount);
    assert_eq!(token_client.balance(&merchant_account_id), 0);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #17)")]
fn test_refund_invoice_fails_after_refund_window() {
    let (env, client, shade_contract_id, admin) = setup_test();
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = create_test_token(&env);
    client.add_accepted_token(&admin, &token);
    let payer = Address::generate(&env);
    let invoice_id = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Expired refund"),
        &500_i128,
        &token,
        &None,
    );

    let merchant_account_id = env.register(MerchantAccount, ());
    let merchant_account = MerchantAccountClient::new(&env, &merchant_account_id);
    merchant_account.initialize(&merchant, &shade_contract_id, &1_u64);

    env.ledger().set_timestamp(604_801);
    mark_invoice_paid(
        &env,
        &shade_contract_id,
        &merchant,
        invoice_id,
        &payer,
        0,
        &merchant_account_id,
        &client,
    );

    client.refund_invoice(&merchant, &invoice_id);
}

// Void Invoice Tests

#[test]
fn test_void_invoice_success() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Test Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token, &None);

    // Verify invoice is Pending
    let invoice_before = client.get_invoice(&invoice_id);
    assert_eq!(invoice_before.status, InvoiceStatus::Pending);

    // Void the invoice
    client.void_invoice(&merchant, &invoice_id);

    // Verify invoice is now Cancelled
    let invoice_after = client.get_invoice(&invoice_id);
    assert_eq!(invoice_after.status, InvoiceStatus::Cancelled);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_refund_invoice_fails_for_non_owner() {
    let (env, client, shade_contract_id, admin) = setup_test();
    let merchant = Address::generate(&env);
    let other_merchant = Address::generate(&env);
    client.register_merchant(&merchant);
    client.register_merchant(&other_merchant);

    let token = create_test_token(&env);
    client.add_accepted_token(&admin, &token);
    let payer = Address::generate(&env);
    let invoice_id = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Wrong owner"),
        &250_i128,
        &token,
        &None,
    );

    let merchant_account_id = env.register(MerchantAccount, ());
    let merchant_account = MerchantAccountClient::new(&env, &merchant_account_id);
    merchant_account.initialize(&merchant, &shade_contract_id, &1_u64);

    env.ledger().set_timestamp(100);
    mark_invoice_paid(
        &env,
        &shade_contract_id,
        &merchant,
        invoice_id,
        &payer,
        90,
        &merchant_account_id,
        &client,
    );

    client.refund_invoice(&other_merchant, &invoice_id);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_void_invoice_non_owner() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Test Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token, &None);

    // Try to void with different merchant (should panic with NotAuthorized)
    let other_merchant = Address::generate(&env);
    client.register_merchant(&other_merchant);
    client.void_invoice(&other_merchant, &invoice_id);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #16)")]
fn test_void_invoice_already_paid() {
    let (env, client, _contract_id, _admin, token) = setup_test_with_payment();

    // Register merchant
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    // Create merchant account
    let merchant_account = Address::generate(&env);
    client.set_merchant_account(&merchant, &merchant_account);

    // Create and pay invoice
    let description = String::from_str(&env, "Test Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token, &None);

    let customer = Address::generate(&env);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    token_client.mint(&customer, &1000);

    client.pay_invoice(&customer, &invoice_id);

    // Try to void paid invoice (should panic with InvalidInvoiceStatus)
    client.void_invoice(&merchant, &invoice_id);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #16)")]
fn test_void_invoice_already_cancelled() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Test Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token, &None);

    // Void the invoice once
    client.void_invoice(&merchant, &invoice_id);

    // Try to void again (should panic with InvalidInvoiceStatus)
    client.void_invoice(&merchant, &invoice_id);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #16)")]
fn test_pay_cancelled_invoice() {
    let (env, client, _contract_id, _admin, token) = setup_test_with_payment();

    // Register merchant
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    // Create merchant account
    let merchant_account = Address::generate(&env);
    client.set_merchant_account(&merchant, &merchant_account);

    // Create invoice
    let description = String::from_str(&env, "Test Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token, &None);

    // Void the invoice
    client.void_invoice(&merchant, &invoice_id);

    // Try to pay cancelled invoice (should panic with InvalidInvoiceStatus)
    let customer = Address::generate(&env);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    token_client.mint(&customer, &1000);

    client.pay_invoice(&customer, &invoice_id);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #8)")]
fn test_void_non_existent_invoice() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    // Try to void non-existent invoice (should panic with InvoiceNotFound)
    client.void_invoice(&merchant, &999);
}

// Invoice Amendment Tests

#[test]
fn test_amend_invoice_amount_success() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Original Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token, &None);

    // Amend the amount
    client.amend_invoice(&merchant, &invoice_id, &Some(2000), &None);

    // Verify amount was updated
    let invoice_after = client.get_invoice(&invoice_id);
    assert_eq!(invoice_after.amount, 2000);
    assert_eq!(invoice_after.description, description);
    assert_eq!(invoice_after.status, InvoiceStatus::Pending);
}

#[test]
fn test_amend_invoice_description_success() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Original Description");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token, &None);

    // Amend the description
    let new_description = String::from_str(&env, "Updated Description");
    client.amend_invoice(
        &merchant,
        &invoice_id,
        &None,
        &Some(new_description.clone()),
    );

    // Verify description was updated
    let invoice_after = client.get_invoice(&invoice_id);
    assert_eq!(invoice_after.amount, 1000);
    assert_eq!(invoice_after.description, new_description);
    assert_eq!(invoice_after.status, InvoiceStatus::Pending);
}

#[test]
fn test_amend_invoice_both_fields_success() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Original");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token, &None);

    // Amend both amount and description
    let new_description = String::from_str(&env, "Updated");
    client.amend_invoice(
        &merchant,
        &invoice_id,
        &Some(3000),
        &Some(new_description.clone()),
    );

    // Verify both fields were updated
    let invoice_after = client.get_invoice(&invoice_id);
    assert_eq!(invoice_after.amount, 3000);
    assert_eq!(invoice_after.description, new_description);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #16)")]
fn test_amend_invoice_paid_fails() {
    let (env, client, _contract_id, _admin, token) = setup_test_with_payment();

    // Register merchant
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    // Create merchant account
    let merchant_account = Address::generate(&env);
    client.set_merchant_account(&merchant, &merchant_account);

    // Create and pay invoice
    let description = String::from_str(&env, "Test Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token, &None);

    let customer = Address::generate(&env);
    let token_client = token::StellarAssetClient::new(&env, &token);
    token_client.mint(&customer, &1000);

    client.pay_invoice(&customer, &invoice_id);

    // Try to amend paid invoice (should panic with InvalidInvoiceStatus)
    let new_description = String::from_str(&env, "Updated");
    client.amend_invoice(&merchant, &invoice_id, &Some(2000), &Some(new_description));
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #16)")]
fn test_amend_invoice_cancelled_fails() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Test Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token, &None);

    // Void the invoice
    client.void_invoice(&merchant, &invoice_id);

    // Try to amend cancelled invoice (should panic with InvalidInvoiceStatus)
    client.amend_invoice(&merchant, &invoice_id, &Some(2000), &None);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_amend_invoice_non_owner_fails() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Test Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token, &None);

    // Try to amend with different merchant (should panic with NotAuthorized)
    let other_merchant = Address::generate(&env);
    client.register_merchant(&other_merchant);
    client.amend_invoice(&other_merchant, &invoice_id, &Some(2000), &None);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #7)")]
fn test_amend_invoice_invalid_amount_fails() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Test Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token, &None);

    // Try to amend with invalid amount (should panic with InvalidAmount)
    client.amend_invoice(&merchant, &invoice_id, &Some(0), &None);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #7)")]
fn test_amend_invoice_negative_amount_fails() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Test Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token, &None);

    // Try to amend with negative amount (should panic with InvalidAmount)
    client.amend_invoice(&merchant, &invoice_id, &Some(-100), &None);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #8)")]
fn test_amend_non_existent_invoice_fails() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    // Try to amend non-existent invoice (should panic with InvoiceNotFound)
    client.amend_invoice(&merchant, &999, &Some(2000), &None);
}

fn setup_test_with_payment() -> (Env, ShadeClient<'static>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let shade_contract_id = env.register(Shade, ());
    let shade_client = ShadeClient::new(&env, &shade_contract_id);

    let admin = Address::generate(&env);
    shade_client.initialize(&admin);

    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(token_admin.clone());

    shade_client.add_accepted_token(&admin, &token.address());
    shade_client.set_fee(&admin, &token.address(), &500);

    (env, shade_client, shade_contract_id, admin, token.address())
}
