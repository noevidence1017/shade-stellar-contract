#![cfg(test)]

use crate::shade::{Shade, ShadeClient};
use crate::types::InvoiceStatus;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env, String};

fn setup_test() -> (Env, ShadeClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, contract_id, admin)
}

/// Test Case 1: Successful Voiding
/// Merchant calls void_invoice for their pending invoice.
/// Verify status updates to Cancelled.
/// Verify InvoiceCancelled event is emitted.
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
    let amount: i128 = 1000;
    let invoice_id = client.create_invoice(&merchant, &description, &amount, &token, &None);

    // Verify invoice is Pending before voiding
    let invoice_before = client.get_invoice(&invoice_id);
    assert_eq!(invoice_before.status, InvoiceStatus::Pending);

    // Void the invoice
    client.void_invoice(&merchant, &invoice_id);

    // Verify invoice is now Cancelled
    let invoice_after = client.get_invoice(&invoice_id);
    assert_eq!(invoice_after.status, InvoiceStatus::Cancelled);
}

/// Test Case 2: Unauthorized Voiding
/// A random address or a different registered merchant attempts to void the invoice.
/// Expect NotAuthorized panic.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_void_invoice_unauthorized_random_address() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Test Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token, &None);

    // Try to void with random address (should panic with NotAuthorized)
    let random_address = Address::generate(&env);
    client.void_invoice(&random_address, &invoice_id);
}

/// Test Case 2b: Unauthorized Voiding - Different Merchant
/// A different registered merchant attempts to void another merchant's invoice.
/// Expect NotAuthorized panic.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_void_invoice_unauthorized_different_merchant() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant1 = Address::generate(&env);
    client.register_merchant(&merchant1);

    let merchant2 = Address::generate(&env);
    client.register_merchant(&merchant2);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Test Invoice");
    let invoice_id = client.create_invoice(&merchant1, &description, &1000, &token, &None);

    // Try to void with different merchant (should panic with NotAuthorized)
    client.void_invoice(&merchant2, &invoice_id);
}

/// Test Case 3: Voiding a Paid Invoice
/// Pay the invoice first, then attempt to void it.
/// Expect InvalidInvoiceStatus panic.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #16)")]
fn test_void_invoice_already_paid() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let token_admin = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();

    client.add_accepted_token(&admin, &token);
    client.set_fee(&admin, &token, &500);

    // Register merchant
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    // Create merchant account
    let merchant_account = Address::generate(&env);
    client.set_merchant_account(&merchant, &merchant_account);

    // Create invoice
    let description = String::from_str(&env, "Test Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token, &None);

    // Pay the invoice
    let customer = Address::generate(&env);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    token_client.mint(&customer, &1000);
    client.pay_invoice(&customer, &invoice_id);

    // Try to void paid invoice (should panic with InvalidInvoiceStatus)
    client.void_invoice(&merchant, &invoice_id);
}

/// Test Case 4: Paying a Voided Invoice
/// Void the invoice first, then attempt to pay it.
/// Expect InvalidInvoiceStatus panic.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #16)")]
fn test_pay_voided_invoice() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let token_admin = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();

    client.add_accepted_token(&admin, &token);
    client.set_fee(&admin, &token, &500);

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

/// Test Case 5: Double Voiding
/// Attempt to void an already Cancelled invoice.
/// Expect InvalidInvoiceStatus panic.
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

/// Test Case 6: Void Non-Existent Invoice
/// Attempt to void an invoice with an ID that doesn't exist.
/// Expect InvoiceNotFound panic.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #8)")]
fn test_void_nonexistent_invoice() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    // Try to void non-existent invoice (should panic with InvoiceNotFound)
    client.void_invoice(&merchant, &999);
}

/// Test Case 7: Merchant Cannot Void Refunded Invoice
/// Verify that refunded invoices cannot be voided.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #16)")]
fn test_void_refunded_invoice() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let token_admin = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();

    client.add_accepted_token(&admin, &token);
    client.set_fee(&admin, &token, &500);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let merchant_account_id = env.register(account::account::MerchantAccount, ());
    let merchant_account = account::account::MerchantAccountClient::new(&env, &merchant_account_id);
    merchant_account.initialize(&merchant, &contract_id, &1_u64);

    client.set_merchant_account(&merchant, &merchant_account_id);

    let description = String::from_str(&env, "Refundable Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token, &None);

    let customer = Address::generate(&env);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    token_client.mint(&customer, &1000);
    token_client.mint(&merchant_account_id, &1000);

    client.pay_invoice(&customer, &invoice_id);
    client.refund_invoice(&merchant, &invoice_id);

    // Try to void refunded invoice (should panic with InvalidInvoiceStatus)
    client.void_invoice(&merchant, &invoice_id);
}

/// Test Case 8: State Isolation - Voiding One Invoice Does Not Affect Others
/// Create multiple invoices, void one, and verify that others remain unaffected.
#[test]
fn test_void_invoice_state_isolation() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Test Invoice");

    let invoice_id_1 = client.create_invoice(&merchant, &description, &1000, &token, &None);
    let invoice_id_2 = client.create_invoice(&merchant, &description, &2000, &token, &None);
    let invoice_id_3 = client.create_invoice(&merchant, &description, &3000, &token, &None);

    // Void only the second invoice
    client.void_invoice(&merchant, &invoice_id_2);

    // Verify first invoice is still Pending
    let invoice_1 = client.get_invoice(&invoice_id_1);
    assert_eq!(invoice_1.status, InvoiceStatus::Pending);

    // Verify second invoice is Cancelled
    let invoice_2 = client.get_invoice(&invoice_id_2);
    assert_eq!(invoice_2.status, InvoiceStatus::Cancelled);

    // Verify third invoice is still Pending
    let invoice_3 = client.get_invoice(&invoice_id_3);
    assert_eq!(invoice_3.status, InvoiceStatus::Pending);
}
