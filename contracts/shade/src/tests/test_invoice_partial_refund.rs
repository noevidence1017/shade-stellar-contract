#![cfg(test)]

use crate::shade::{Shade, ShadeClient};
use crate::types::{DataKey, InvoiceStatus};
use account::account::{MerchantAccount, MerchantAccountClient};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{token, Address, Env, String};

const INVOICE_AMOUNT: i128 = 1_000;
const REFUND_WINDOW_SECS: u64 = 604_800;

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

#[allow(clippy::too_many_arguments)]
fn setup_paid_invoice(
    env: &Env,
    client: &ShadeClient<'_>,
    shade_contract_id: &Address,
    merchant: &Address,
    payer: &Address,
    token: &Address,
    amount: i128,
    date_paid: u64,
) -> (u64, Address) {
    let invoice_id = client.create_invoice(
        merchant,
        &String::from_str(env, "Partial refund test"),
        &amount,
        token,
        &None,
    );

    let merchant_account_id = env.register(MerchantAccount, ());
    let merchant_account = MerchantAccountClient::new(env, &merchant_account_id);
    merchant_account.initialize(merchant, shade_contract_id, &1_u64);
    client.set_merchant_account(merchant, &merchant_account_id);

    let token_admin = token::StellarAssetClient::new(env, token);
    token_admin.mint(&merchant_account_id, &amount);

    env.ledger().set_timestamp(date_paid.saturating_add(1));
    let mut invoice = client.get_invoice(&invoice_id);
    invoice.status = InvoiceStatus::Paid;
    invoice.payer = Some(payer.clone());
    invoice.date_paid = Some(date_paid);

    env.as_contract(shade_contract_id, || {
        env.storage()
            .persistent()
            .set(&DataKey::Invoice(invoice_id), &invoice);
    });

    (invoice_id, merchant_account_id)
}

#[test]
fn test_partial_refund_single_balance_and_status() {
    let (env, client, shade_contract_id, admin) = setup_test();
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);
    let token = create_test_token(&env);
    client.add_accepted_token(&admin, &token);
    let payer = Address::generate(&env);

    let (invoice_id, merchant_account_id) = setup_paid_invoice(
        &env,
        &client,
        &shade_contract_id,
        &merchant,
        &payer,
        &token,
        INVOICE_AMOUNT,
        1_000,
    );

    let token_client = token::TokenClient::new(&env, &token);
    let merchant_balance_before = token_client.balance(&merchant_account_id);
    let payer_balance_before = token_client.balance(&payer);
    assert_eq!(merchant_balance_before, INVOICE_AMOUNT);
    assert_eq!(payer_balance_before, 0);

    client.refund_invoice_partial(&invoice_id, &300);

    let invoice = client.get_invoice(&invoice_id);
    assert_eq!(invoice.status, InvoiceStatus::PartiallyRefunded);
    assert_eq!(invoice.amount_refunded, 300);

    assert_eq!(
        token_client.balance(&merchant_account_id),
        merchant_balance_before - 300
    );
    assert_eq!(token_client.balance(&payer), payer_balance_before + 300);
}

#[test]
fn test_partial_refund_multiple_accumulates() {
    let (env, client, shade_contract_id, admin) = setup_test();
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);
    let token = create_test_token(&env);
    client.add_accepted_token(&admin, &token);
    let payer = Address::generate(&env);

    let (invoice_id, merchant_account_id) = setup_paid_invoice(
        &env,
        &client,
        &shade_contract_id,
        &merchant,
        &payer,
        &token,
        INVOICE_AMOUNT,
        1_000,
    );

    client.refund_invoice_partial(&invoice_id, &200);
    let invoice = client.get_invoice(&invoice_id);
    assert_eq!(invoice.status, InvoiceStatus::PartiallyRefunded);
    assert_eq!(invoice.amount_refunded, 200);

    client.refund_invoice_partial(&invoice_id, &400);
    let invoice = client.get_invoice(&invoice_id);
    assert_eq!(invoice.status, InvoiceStatus::PartiallyRefunded);
    assert_eq!(invoice.amount_refunded, 600);

    let token_client = token::TokenClient::new(&env, &token);
    assert_eq!(token_client.balance(&merchant_account_id), 400);
    assert_eq!(token_client.balance(&payer), 600);
}

#[test]
fn test_partial_refund_full_via_partial_transitions_to_refunded() {
    let (env, client, shade_contract_id, admin) = setup_test();
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);
    let token = create_test_token(&env);
    client.add_accepted_token(&admin, &token);
    let payer = Address::generate(&env);

    let (invoice_id, merchant_account_id) = setup_paid_invoice(
        &env,
        &client,
        &shade_contract_id,
        &merchant,
        &payer,
        &token,
        INVOICE_AMOUNT,
        1_000,
    );

    client.refund_invoice_partial(&invoice_id, &300);
    client.refund_invoice_partial(&invoice_id, &300);
    client.refund_invoice_partial(&invoice_id, &400);

    let invoice = client.get_invoice(&invoice_id);
    assert_eq!(invoice.status, InvoiceStatus::Refunded);
    assert_eq!(invoice.amount_refunded, INVOICE_AMOUNT);

    let token_client = token::TokenClient::new(&env, &token);
    assert_eq!(token_client.balance(&merchant_account_id), 0);
    assert_eq!(token_client.balance(&payer), INVOICE_AMOUNT);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #7)")]
fn test_partial_refund_over_refund_panics() {
    let (env, client, shade_contract_id, admin) = setup_test();
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);
    let token = create_test_token(&env);
    client.add_accepted_token(&admin, &token);
    let payer = Address::generate(&env);

    let (invoice_id, _) = setup_paid_invoice(
        &env,
        &client,
        &shade_contract_id,
        &merchant,
        &payer,
        &token,
        INVOICE_AMOUNT,
        1_000,
    );

    client.refund_invoice_partial(&invoice_id, &600);
    client.refund_invoice_partial(&invoice_id, &500);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #17)")]
fn test_partial_refund_fails_after_seven_days() {
    let (env, client, shade_contract_id, admin) = setup_test();
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);
    let token = create_test_token(&env);
    client.add_accepted_token(&admin, &token);
    let payer = Address::generate(&env);

    let date_paid = 0;
    let (invoice_id, _) = setup_paid_invoice(
        &env,
        &client,
        &shade_contract_id,
        &merchant,
        &payer,
        &token,
        INVOICE_AMOUNT,
        date_paid,
    );

    env.ledger()
        .set_timestamp(date_paid + REFUND_WINDOW_SECS + 1);
    client.refund_invoice_partial(&invoice_id, &300);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #7)")]
fn test_partial_refund_zero_amount_panics() {
    let (env, client, shade_contract_id, admin) = setup_test();
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);
    let token = create_test_token(&env);
    client.add_accepted_token(&admin, &token);
    let payer = Address::generate(&env);

    let (invoice_id, _) = setup_paid_invoice(
        &env,
        &client,
        &shade_contract_id,
        &merchant,
        &payer,
        &token,
        INVOICE_AMOUNT,
        1_000,
    );

    client.refund_invoice_partial(&invoice_id, &0);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #7)")]
fn test_partial_refund_negative_amount_panics() {
    let (env, client, shade_contract_id, admin) = setup_test();
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);
    let token = create_test_token(&env);
    client.add_accepted_token(&admin, &token);
    let payer = Address::generate(&env);

    let (invoice_id, _) = setup_paid_invoice(
        &env,
        &client,
        &shade_contract_id,
        &merchant,
        &payer,
        &token,
        INVOICE_AMOUNT,
        1_000,
    );

    client.refund_invoice_partial(&invoice_id, &-100);
}
