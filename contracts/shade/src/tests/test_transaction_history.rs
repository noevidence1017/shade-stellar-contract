#![cfg(test)]

use crate::shade::{Shade, ShadeClient};
use crate::types::TransactionType;
use soroban_sdk::testutils::{Address as _, Ledger as _};
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
    shade_client.set_fee(&admin, &token.address(), &500); // 5%

    (env, shade_client, shade_contract_id, admin, token.address())
}

#[test]
fn test_invoice_payment_records_history() {
    let (env, shade_client, _shade_contract_id, _admin, token) = setup_test();

    let merchant = Address::generate(&env);
    shade_client.register_merchant(&merchant);
    let merchant_account = Address::generate(&env);
    shade_client.set_merchant_account(&merchant, &merchant_account);

    let description = String::from_str(&env, "History Test");
    let invoice_id = shade_client.create_invoice(&merchant, &description, &1000, &token, &None);

    let customer = Address::generate(&env);
    let token_client = token::StellarAssetClient::new(&env, &token);
    token_client.mint(&customer, &1000);

    shade_client.pay_invoice(&customer, &invoice_id);

    let history = shade_client.get_user_transactions(&customer);
    assert_eq!(history.len(), 1);

    let tx = history.get(0).unwrap();
    assert_eq!(tx.transaction_type, TransactionType::InvoicePayment);
    assert_eq!(tx.ref_id, invoice_id);
    assert_eq!(tx.amount, 1000);
    assert_eq!(tx.token, token);
    assert_eq!(tx.description, description);
    assert_eq!(tx.merchant_id, 1);
}

#[test]
fn test_multiple_payments_record_history() {
    let (env, shade_client, _shade_contract_id, _admin, token) = setup_test();

    let merchant = Address::generate(&env);
    shade_client.register_merchant(&merchant);
    let merchant_account = Address::generate(&env);
    shade_client.set_merchant_account(&merchant, &merchant_account);

    let customer = Address::generate(&env);
    let token_client = token::StellarAssetClient::new(&env, &token);
    token_client.mint(&customer, &5000);

    for _i in 1..=3 {
        let description = String::from_str(&env, "Test Inv");
        let invoice_id = shade_client.create_invoice(&merchant, &description, &1000, &token, &None);
        shade_client.pay_invoice(&customer, &invoice_id);
    }

    let history = shade_client.get_user_transactions(&customer);
    assert_eq!(history.len(), 3);
}

#[test]
fn test_subscription_charge_records_history() {
    let (env, shade_client, _shade_contract_id, _admin, token) = setup_test();

    let merchant = Address::generate(&env);
    shade_client.register_merchant(&merchant);
    let merchant_account = Address::generate(&env);
    shade_client.set_merchant_account(&merchant, &merchant_account);

    let plan_desc = String::from_str(&env, "Plan History");
    let plan_id = shade_client.create_subscription_plan(&merchant, &plan_desc, &token, &100, &3600);

    let customer = Address::generate(&env);
    let token_client = token::StellarAssetClient::new(&env, &token);
    token_client.mint(&customer, &1000);
    
    // Manual approval for subscription
    let token_token_client = token::TokenClient::new(&env, &token);
    token_token_client.approve(&customer, &shade_client.address, &1000, &2000);

    let sub_id = shade_client.subscribe(&customer, &plan_id);
    shade_client.charge_subscription(&sub_id);

    let history = shade_client.get_user_transactions(&customer);
    assert_eq!(history.len(), 1);

    let tx = history.get(0).unwrap();
    assert_eq!(tx.transaction_type, TransactionType::SubscriptionCharge);
    assert_eq!(tx.ref_id, sub_id);
    assert_eq!(tx.amount, 100);
    assert_eq!(tx.token, token);
    assert_eq!(tx.description, plan_desc);
}
