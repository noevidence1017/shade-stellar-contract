#![cfg(test)]

use crate::shade::{Shade, ShadeClient};
use crate::types::InvoiceStatus;
use account::account::{MerchantAccount, MerchantAccountClient};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{token, Address, Env, String};

/// Shared setup: deploy Shade, initialize, register a token with **0 fee**,
/// register a merchant, deploy + link a merchant account, create an invoice,
/// mint tokens to the customer, and pay the invoice at the given timestamp.
///
/// Fee is intentionally 0 so the full `invoice.amount` lands in the merchant
/// account and `refund_invoice` (which refunds the full amount from the
/// merchant account) can succeed.  Fee-related interactions are covered by a
/// dedicated test.
struct RefundTestContext<'a> {
    env: Env,
    client: ShadeClient<'a>,
    shade_id: Address,
    admin: Address,
    merchant: Address,
    merchant_account_id: Address,
    token: Address,
    payer: Address,
    invoice_id: u64,
    amount: i128,
}

fn setup_paid_invoice(pay_timestamp: u64) -> RefundTestContext<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let shade_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &shade_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    // Register token with 0 fee – keeps full amount in merchant account
    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token = token_contract.address();
    client.add_accepted_token(&admin, &token);
    client.set_fee(&admin, &token, &0);

    // Register merchant + deploy merchant account contract
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let merchant_account_id = env.register(MerchantAccount, ());
    let merchant_account = MerchantAccountClient::new(&env, &merchant_account_id);
    merchant_account.initialize(&merchant, &shade_id, &1_u64);
    client.set_merchant_account(&merchant, &merchant_account_id);

    // Create invoice
    let amount = 1_000_i128;
    let description = String::from_str(&env, "Refund Test Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &amount, &token, &None);

    // Mint tokens to the payer and pay the invoice
    let payer = Address::generate(&env);
    let token_mint = token::StellarAssetClient::new(&env, &token);
    token_mint.mint(&payer, &amount);

    env.ledger().set_timestamp(pay_timestamp);
    client.pay_invoice(&payer, &invoice_id);

    RefundTestContext {
        env,
        client,
        shade_id,
        admin,
        merchant,
        merchant_account_id,
        token,
        payer,
        invoice_id,
        amount,
    }
}

// ---------------------------------------------------------------------------
// Test Case 1: Successful Refund (within 7-day window)
// Refund the invoice 1 hour after payment.
// Verify the payer receives the full amount back and status is Refunded.
// ---------------------------------------------------------------------------
#[test]
fn test_refund_successful_within_window() {
    let ctx = setup_paid_invoice(1_000);

    // Advance ledger by 1 hour (3 600 seconds) from the payment time
    ctx.env.ledger().set_timestamp(1_000 + 3_600);

    ctx.client.refund_invoice(&ctx.merchant, &ctx.invoice_id);

    let invoice = ctx.client.get_invoice(&ctx.invoice_id);
    assert_eq!(invoice.status, InvoiceStatus::Refunded);
    assert_eq!(invoice.amount_refunded, ctx.amount);

    // With 0% fee the full amount was in the merchant account and is now
    // transferred back to the payer.
    let tok = token::TokenClient::new(&ctx.env, &ctx.token);
    assert_eq!(tok.balance(&ctx.payer), ctx.amount);
    assert_eq!(tok.balance(&ctx.merchant_account_id), 0);
}

// ---------------------------------------------------------------------------
// Test Case 2: Refund Period Expiry
// Increase the ledger timestamp by 8 days. Attempt to refund.
// Expect RefundPeriodExpired (#17).
// ---------------------------------------------------------------------------
#[test]
#[should_panic(expected = "HostError: Error(Contract, #17)")]
fn test_refund_fails_after_7_day_window() {
    let ctx = setup_paid_invoice(1_000);

    // 8 days = 691 200 seconds → well past the 604 800-second window
    ctx.env.ledger().set_timestamp(1_000 + 691_200);

    ctx.client.refund_invoice(&ctx.merchant, &ctx.invoice_id);
}

// ---------------------------------------------------------------------------
// Test Case 2b: Refund at exact boundary (604 800 seconds) should still succeed
// ---------------------------------------------------------------------------
#[test]
fn test_refund_at_exact_boundary_succeeds() {
    let ctx = setup_paid_invoice(1_000);

    ctx.env.ledger().set_timestamp(1_000 + 604_800);

    ctx.client.refund_invoice(&ctx.merchant, &ctx.invoice_id);

    let invoice = ctx.client.get_invoice(&ctx.invoice_id);
    assert_eq!(invoice.status, InvoiceStatus::Refunded);
}

// ---------------------------------------------------------------------------
// Test Case 2c: Refund 1 second past boundary should fail
// ---------------------------------------------------------------------------
#[test]
#[should_panic(expected = "HostError: Error(Contract, #17)")]
fn test_refund_one_second_past_boundary_fails() {
    let ctx = setup_paid_invoice(1_000);

    ctx.env.ledger().set_timestamp(1_000 + 604_801);

    ctx.client.refund_invoice(&ctx.merchant, &ctx.invoice_id);
}

// ---------------------------------------------------------------------------
// Test Case 3: Unauthorized Refund – random address
// A random address attempts to refund the invoice.
// Expect NotAuthorized (#1).
// ---------------------------------------------------------------------------
#[test]
#[should_panic(expected = "HostError: Error(Contract, #6)")]
fn test_refund_unauthorized_random_address() {
    let ctx = setup_paid_invoice(1_000);

    ctx.env.ledger().set_timestamp(1_000 + 3_600);

    let random = Address::generate(&ctx.env);
    ctx.client.refund_invoice(&random, &ctx.invoice_id);
}

// ---------------------------------------------------------------------------
// Test Case 3b: Unauthorized Refund – different merchant
// A different registered merchant attempts to refund anothers invoice.
// Expect NotAuthorized (#1).
// ---------------------------------------------------------------------------
#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_refund_unauthorized_different_merchant() {
    let ctx = setup_paid_invoice(1_000);

    ctx.env.ledger().set_timestamp(1_000 + 3_600);

    let other_merchant = Address::generate(&ctx.env);
    ctx.client.register_merchant(&other_merchant);

    ctx.client.refund_invoice(&other_merchant, &ctx.invoice_id);
}

// ---------------------------------------------------------------------------
// Test Case 4: Restricted Account Refund
// Restrict the merchants account contract first.
// Attempt to refund via the Shade contract.
// Expect the AccountRestricted error to propagate (#5 from account contract).
// ---------------------------------------------------------------------------
#[test]
#[should_panic(expected = "HostError: Error(Contract, #5)")]
fn test_refund_restricted_merchant_account() {
    let ctx = setup_paid_invoice(1_000);

    ctx.env.ledger().set_timestamp(1_000 + 3_600);

    // Restrict the merchant account via the Shade contracts admin flow
    ctx.client
        .restrict_merchant_account(&ctx.admin, &ctx.merchant, &true);

    ctx.client.refund_invoice(&ctx.merchant, &ctx.invoice_id);
}

// ---------------------------------------------------------------------------
// Test Case 5a: Invalid Invoice Status – Pending invoice
// Attempt to refund an invoice that is still Pending (never paid).
// Expect InvalidInvoiceStatus (#16).
// ---------------------------------------------------------------------------
#[test]
#[should_panic(expected = "HostError: Error(Contract, #29)")]
fn test_refund_pending_invoice_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let shade_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &shade_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Never Paid");
    let invoice_id = client.create_invoice(&merchant, &description, &500, &token, &None);

    // Invoice is Pending – refund should fail
    client.refund_invoice(&merchant, &invoice_id);
}

// ---------------------------------------------------------------------------
// Test Case 5b: Invalid Invoice Status – Cancelled invoice
// Cancel the invoice first, then attempt to refund it.
// Expect InvalidInvoiceStatus (#16).
// ---------------------------------------------------------------------------
#[test]
#[should_panic(expected = "HostError: Error(Contract, #29)")]
fn test_refund_cancelled_invoice_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let shade_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &shade_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Cancel Me");
    let invoice_id = client.create_invoice(&merchant, &description, &500, &token, &None);

    client.void_invoice(&merchant, &invoice_id);

    // Invoice is now Cancelled – refund should fail
    client.refund_invoice(&merchant, &invoice_id);
}

// ---------------------------------------------------------------------------
// Test Case 6: Double Refund – attempt to refund an already-refunded invoice
// The second call hits `amount_to_refund <= 0` → InvalidAmount (#7).
// ---------------------------------------------------------------------------
#[test]
#[should_panic(expected = "HostError: Error(Contract, #28)")]
fn test_double_refund_fails() {
    let ctx = setup_paid_invoice(1_000);

    ctx.env.ledger().set_timestamp(1_000 + 3_600);

    // First refund should succeed
    ctx.client.refund_invoice(&ctx.merchant, &ctx.invoice_id);
    let invoice = ctx.client.get_invoice(&ctx.invoice_id);
    assert_eq!(invoice.status, InvoiceStatus::Refunded);

    // Second refund should fail – already fully refunded
    ctx.client.refund_invoice(&ctx.merchant, &ctx.invoice_id);
}

// ---------------------------------------------------------------------------
// Test Case 7: Fund movement verification (detailed balance check)
// Verify the exact token movements with 0% fee:
// Payment: payer → merchant_account (full amount)
// Refund:  merchant_account → payer (full amount)
// ---------------------------------------------------------------------------
#[test]
fn test_refund_fund_movement_detailed() {
    let ctx = setup_paid_invoice(1_000);

    let tok = token::TokenClient::new(&ctx.env, &ctx.token);

    // After payment (0% fee): payer = 0, merchant_account = 1000, shade = 0
    assert_eq!(tok.balance(&ctx.payer), 0);
    assert_eq!(tok.balance(&ctx.merchant_account_id), ctx.amount);
    assert_eq!(tok.balance(&ctx.shade_id), 0);

    ctx.env.ledger().set_timestamp(1_000 + 3_600);
    ctx.client.refund_invoice(&ctx.merchant, &ctx.invoice_id);

    // After refund: payer = 1000, merchant_account = 0, shade = 0
    assert_eq!(tok.balance(&ctx.payer), ctx.amount);
    assert_eq!(tok.balance(&ctx.merchant_account_id), 0);
    assert_eq!(tok.balance(&ctx.shade_id), 0);
}

// ---------------------------------------------------------------------------
// Test Case 8: Partial refund with fee
// Set a 5% fee, pay, then manually do a partial refund of the merchant
// portion only.  This validates that refund_invoice_partial works correctly
// with the amount that the merchant account actually holds.
// ---------------------------------------------------------------------------
#[test]
fn test_partial_refund_with_fee() {
    let env = Env::default();
    env.mock_all_auths();

    let shade_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &shade_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token = token_contract.address();
    client.add_accepted_token(&admin, &token);
    client.set_fee(&admin, &token, &500); // 5% fee

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let merchant_account_id = env.register(MerchantAccount, ());
    let merchant_account = MerchantAccountClient::new(&env, &merchant_account_id);
    merchant_account.initialize(&merchant, &shade_id, &1_u64);
    client.set_merchant_account(&merchant, &merchant_account_id);

    let amount = 1_000_i128;
    let description = String::from_str(&env, "Fee Refund");
    let invoice_id = client.create_invoice(&merchant, &description, &amount, &token, &None);

    let payer = Address::generate(&env);
    let token_mint = token::StellarAssetClient::new(&env, &token);
    token_mint.mint(&payer, &amount);

    env.ledger().set_timestamp(1_000);
    client.pay_invoice(&payer, &invoice_id);

    let tok = token::TokenClient::new(&env, &token);
    let fee = amount * 500 / 10_000; // 50
    let merchant_portion = amount - fee; // 950

    assert_eq!(tok.balance(&merchant_account_id), merchant_portion);
    assert_eq!(tok.balance(&shade_id), fee);

    // Partial refund of exactly the merchant portion
    env.ledger().set_timestamp(1_000 + 3_600);
    client.refund_invoice_partial(&invoice_id, &merchant_portion);

    let invoice = client.get_invoice(&invoice_id);
    assert_eq!(invoice.status, InvoiceStatus::PartiallyRefunded);
    assert_eq!(invoice.amount_refunded, merchant_portion);

    assert_eq!(tok.balance(&payer), merchant_portion);
    assert_eq!(tok.balance(&merchant_account_id), 0);
    assert_eq!(tok.balance(&shade_id), fee); // fee stays with shade
}
