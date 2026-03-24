#![cfg(test)]

use crate::shade::{Shade, ShadeClient};
use crate::types::InvoiceStatus;
use account::account::MerchantAccount;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{token, Address, Env, String};

fn setup_test() -> (Env, ShadeClient<'static>, Address, Address, Address) {
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

#[test]
fn test_create_draft_invoice() {
    let (env, client, _contract_id, _admin, token) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let description = String::from_str(&env, "Draft Invoice");
    let amount: i128 = 1000;

    let invoice_id = client.create_invoice_draft(&merchant, &description, &amount, &token, &None);
    assert_eq!(invoice_id, 1);

    let invoice = client.get_invoice(&invoice_id);

    assert_eq!(invoice.id, 1);
    assert_eq!(invoice.merchant_id, 1);
    assert_eq!(invoice.amount, amount);
    assert_eq!(invoice.token, token);
    assert_eq!(invoice.description, description);
    assert_eq!(invoice.status, InvoiceStatus::Draft);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #16)")]
fn test_pay_draft_invoice_fails() {
    let (env, client, _contract_id, _admin, token) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let description = String::from_str(&env, "Draft Invoice");
    let invoice_id = client.create_invoice_draft(&merchant, &description, &1000, &token, &None);

    let customer = Address::generate(&env);
    let token_client = token::StellarAssetClient::new(&env, &token);
    token_client.mint(&customer, &1000);

    // Try to pay draft invoice (should panic with InvalidInvoiceStatus = #16)
    client.pay_invoice(&customer, &invoice_id);
}

#[test]
fn test_finalize_invoice_success() {
    let (env, client, _contract_id, _admin, token) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let description = String::from_str(&env, "Draft Invoice");
    let invoice_id = client.create_invoice_draft(&merchant, &description, &1000, &token, &None);

    // Verify invoice is Draft
    let invoice_before = client.get_invoice(&invoice_id);
    assert_eq!(invoice_before.status, InvoiceStatus::Draft);

    // Finalize the invoice
    client.finalize_invoice(&merchant, &invoice_id);

    // Verify invoice is now Pending
    let invoice_after = client.get_invoice(&invoice_id);
    assert_eq!(invoice_after.status, InvoiceStatus::Pending);
}

#[test]
fn test_pay_finalized_invoice_success() {
    let (env, client, shade_contract_id, _admin, token) = setup_test();

    // Register merchant
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    // Create merchant account
    let merchant_account = env.register(MerchantAccount, ());
    client.set_merchant_account(&merchant, &merchant_account);
    use account::account::MerchantAccountClient;
    let merchant_account_client = MerchantAccountClient::new(&env, &merchant_account);
    merchant_account_client.initialize(&merchant, &shade_contract_id, &1_u64);

    // Create draft invoice
    let description = String::from_str(&env, "Draft Invoice");
    let invoice_id = client.create_invoice_draft(&merchant, &description, &1000, &token, &None);

    // Finalize invoice
    client.finalize_invoice(&merchant, &invoice_id);

    let customer = Address::generate(&env);
    let token_admin = token::StellarAssetClient::new(&env, &token);
    token_admin.mint(&customer, &1000);

    // Pay invoice
    client.pay_invoice(&customer, &invoice_id);

    let invoice_after = client.get_invoice(&invoice_id);
    assert_eq!(invoice_after.status, InvoiceStatus::Paid);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_finalize_invoice_non_owner_fails() {
    let (env, client, _contract_id, _admin, token) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let description = String::from_str(&env, "Draft Invoice");
    let invoice_id = client.create_invoice_draft(&merchant, &description, &1000, &token, &None);

    let other_merchant = Address::generate(&env);
    client.register_merchant(&other_merchant);

    // Try to finalize with different merchant (should panic with NotAuthorized = #1)
    client.finalize_invoice(&other_merchant, &invoice_id);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #16)")]
fn test_finalize_non_draft_invoice_fails() {
    let (env, client, _contract_id, _admin, token) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let description = String::from_str(&env, "Standard Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token, &None);

    // Try to finalize a standard Pending invoice (should panic with InvalidInvoiceStatus = #16)
    client.finalize_invoice(&merchant, &invoice_id);
}
