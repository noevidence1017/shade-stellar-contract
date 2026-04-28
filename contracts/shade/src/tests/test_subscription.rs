#![cfg(test)]

use crate::shade::{Shade, ShadeClient};
use crate::types::SubscriptionStatus;
use account::account::{MerchantAccount, MerchantAccountClient};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{token, Address, Env, String};

/// Monthly interval constant: 30 days = 2 592 000 seconds.
const MONTHLY_INTERVAL: u64 = 2_592_000;

/// Shared test context for subscription tests.
struct SubTestContext<'a> {
    env: Env,
    client: ShadeClient<'a>,
    shade_id: Address,
    admin: Address,
    merchant: Address,
    merchant_account_id: Address,
    token: Address,
    plan_id: u64,
}

fn setup_subscription_env() -> SubTestContext<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let shade_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &shade_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    // Register token with 5% fee
    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_addr = token_contract.address();
    client.add_accepted_token(&admin, &token_addr);
    client.set_fee(&admin, &token_addr, &500);

    // Register merchant + merchant account
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let merchant_account_id = env.register(MerchantAccount, ());
    let merchant_account = MerchantAccountClient::new(&env, &merchant_account_id);
    merchant_account.initialize(&merchant, &shade_id, &1_u64);
    client.set_merchant_account(&merchant, &merchant_account_id);

    // Merchant creates a monthly plan
    let description = String::from_str(&env, "Monthly Pro Plan");
    let amount = 1_000_i128;
    let plan_id = client.create_subscription_plan(
        &merchant,
        &description,
        &token_addr,
        &amount,
        &MONTHLY_INTERVAL,
    );

    SubTestContext {
        env,
        client,
        shade_id,
        admin,
        merchant,
        merchant_account_id,
        token: token_addr,
        plan_id,
    }
}

/// Mint tokens to a customer and approve the shade contract to spend them.
fn fund_and_approve(ctx: &SubTestContext, customer: &Address, mint_amount: i128) {
    let token_mint = token::StellarAssetClient::new(&ctx.env, &ctx.token);
    token_mint.mint(customer, &mint_amount);
    // Approve the shade contract to spend on behalf of the customer
    let tok = token::TokenClient::new(&ctx.env, &ctx.token);
    tok.approve(customer, &ctx.shade_id, &mint_amount, &1_000_000);
}

// ---------------------------------------------------------------------------
// Test Case 1: Subscription Lifecycle
// Merchant creates a plan, customer subscribes, records are correctly init.
// ---------------------------------------------------------------------------
#[test]
fn test_subscription_lifecycle() {
    let ctx = setup_subscription_env();

    // Verify plan stored correctly
    let plan = ctx.client.get_subscription_plan(&ctx.plan_id);
    assert_eq!(plan.amount, 1_000);
    assert_eq!(plan.interval, MONTHLY_INTERVAL);
    assert!(plan.active);

    // Customer subscribes
    let customer = Address::generate(&ctx.env);
    let sub_id = ctx.client.subscribe(&customer, &ctx.plan_id);

    let sub = ctx.client.get_subscription(&sub_id);
    assert_eq!(sub.plan_id, ctx.plan_id);
    assert_eq!(sub.customer, customer);
    assert_eq!(sub.status, SubscriptionStatus::Active);
    assert_eq!(sub.last_charged, 0);
}

// ---------------------------------------------------------------------------
// Test Case 2: Successful Charge
// Customer approves, wait for interval, call charge_subscription.
// Verify token distribution (fee to Shade, net to merchant account).
// ---------------------------------------------------------------------------
#[test]
fn test_successful_charge() {
    let ctx = setup_subscription_env();

    let customer = Address::generate(&ctx.env);
    fund_and_approve(&ctx, &customer, 10_000);

    let sub_id = ctx.client.subscribe(&customer, &ctx.plan_id);

    // First charge is immediate (last_charged == 0)
    ctx.client.charge_subscription(&sub_id);

    let tok = token::TokenClient::new(&ctx.env, &ctx.token);
    let fee = 1_000 * 500 / 10_000; // 50
    let merchant_portion = 1_000 - fee; // 950

    assert_eq!(tok.balance(&ctx.merchant_account_id), merchant_portion);
    assert_eq!(tok.balance(&ctx.admin), fee);
    assert_eq!(tok.balance(&customer), 10_000 - 1_000);

    // Verify last_charged updated
    let sub = ctx.client.get_subscription(&sub_id);
    assert_eq!(sub.last_charged, ctx.env.ledger().timestamp());
}

// ---------------------------------------------------------------------------
// Test Case 3: Premature Charge Prevention
// Attempt to charge again immediately after the first charge.
// Expect ChargeTooEarly (#26).
// ---------------------------------------------------------------------------
#[test]
#[should_panic(expected = "HostError: Error(Contract, #26)")]
fn test_premature_charge_fails() {
    let ctx = setup_subscription_env();

    let customer = Address::generate(&ctx.env);
    fund_and_approve(&ctx, &customer, 10_000);

    let sub_id = ctx.client.subscribe(&customer, &ctx.plan_id);

    ctx.env.ledger().set_timestamp(100);
    ctx.client.charge_subscription(&sub_id);

    // Same timestamp – interval has not passed
    ctx.client.charge_subscription(&sub_id);
}

// ---------------------------------------------------------------------------
// Test Case 3b: Charge succeeds after full interval elapses
// ---------------------------------------------------------------------------
#[test]
fn test_charge_after_interval() {
    let ctx = setup_subscription_env();

    let customer = Address::generate(&ctx.env);
    fund_and_approve(&ctx, &customer, 10_000);

    let sub_id = ctx.client.subscribe(&customer, &ctx.plan_id);

    ctx.env.ledger().set_timestamp(100);
    ctx.client.charge_subscription(&sub_id);

    // Advance by exactly the interval
    ctx.env.ledger().set_timestamp(100 + MONTHLY_INTERVAL);
    ctx.client.charge_subscription(&sub_id);

    let tok = token::TokenClient::new(&ctx.env, &ctx.token);
    // Two charges: 2 × 1000 = 2000 deducted from customer
    assert_eq!(tok.balance(&customer), 10_000 - 2_000);

    let fee = 1_000 * 500 / 10_000; // 50 per charge
    assert_eq!(tok.balance(&ctx.admin), fee * 2);
    assert_eq!(tok.balance(&ctx.merchant_account_id), (1_000 - fee) * 2);
}

// ---------------------------------------------------------------------------
// Test Case 4: Allowance / Balance Failure
// Customer has insufficient balance. Expect token transfer panic.
// ---------------------------------------------------------------------------
#[test]
#[should_panic]
fn test_charge_insufficient_balance() {
    let ctx = setup_subscription_env();

    let customer = Address::generate(&ctx.env);
    // Mint and approve less than the plan amount
    fund_and_approve(&ctx, &customer, 100);

    let sub_id = ctx.client.subscribe(&customer, &ctx.plan_id);

    // Charge should fail because customer only has 100 but plan costs 1000
    ctx.client.charge_subscription(&sub_id);
}

// ---------------------------------------------------------------------------
// Test Case 5: Cancellation by Merchant
// Merchant cancels the subscription.
// Verify that subsequent charges fail.
// ---------------------------------------------------------------------------
#[test]
fn test_merchant_cancels_subscription() {
    let ctx = setup_subscription_env();

    let customer = Address::generate(&ctx.env);
    let sub_id = ctx.client.subscribe(&customer, &ctx.plan_id);

    ctx.client.cancel_subscription(&ctx.merchant, &sub_id);

    let sub = ctx.client.get_subscription(&sub_id);
    assert_eq!(sub.status, SubscriptionStatus::Cancelled);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #25)")]
fn test_charge_cancelled_subscription_fails() {
    let ctx = setup_subscription_env();

    let customer = Address::generate(&ctx.env);
    fund_and_approve(&ctx, &customer, 10_000);

    let sub_id = ctx.client.subscribe(&customer, &ctx.plan_id);

    ctx.client.cancel_subscription(&ctx.merchant, &sub_id);

    // Should fail – subscription is cancelled
    ctx.client.charge_subscription(&sub_id);
}

// ---------------------------------------------------------------------------
// Test Case 5b: Cancellation by Customer
// ---------------------------------------------------------------------------
#[test]
fn test_customer_cancels_subscription() {
    let ctx = setup_subscription_env();

    let customer = Address::generate(&ctx.env);
    let sub_id = ctx.client.subscribe(&customer, &ctx.plan_id);

    ctx.client.cancel_subscription(&customer, &sub_id);

    let sub = ctx.client.get_subscription(&sub_id);
    assert_eq!(sub.status, SubscriptionStatus::Cancelled);
}

// ---------------------------------------------------------------------------
// Test Case 5c: Cancellation by unauthorized party fails
// ---------------------------------------------------------------------------
#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_cancel_by_unauthorized_fails() {
    let ctx = setup_subscription_env();

    let customer = Address::generate(&ctx.env);
    let sub_id = ctx.client.subscribe(&customer, &ctx.plan_id);

    let random = Address::generate(&ctx.env);
    ctx.client.cancel_subscription(&random, &sub_id);
}

// ---------------------------------------------------------------------------
// Test Case 6: Role-Based Execution – anyone can call charge_subscription
// (incentivized billing: a third party triggers the charge)
// ---------------------------------------------------------------------------
#[test]
fn test_anyone_can_trigger_charge() {
    let ctx = setup_subscription_env();

    let customer = Address::generate(&ctx.env);
    fund_and_approve(&ctx, &customer, 10_000);

    let sub_id = ctx.client.subscribe(&customer, &ctx.plan_id);

    // A completely unrelated address triggers the charge
    ctx.client.charge_subscription(&sub_id);

    let tok = token::TokenClient::new(&ctx.env, &ctx.token);
    assert_eq!(tok.balance(&customer), 10_000 - 1_000);
}

// ---------------------------------------------------------------------------
// Test Case 7: Multi-cycle recurring charges
// Simulate 3 consecutive monthly charges.
// ---------------------------------------------------------------------------
#[test]
fn test_multi_cycle_charges() {
    let ctx = setup_subscription_env();

    let customer = Address::generate(&ctx.env);
    fund_and_approve(&ctx, &customer, 10_000);

    let sub_id = ctx.client.subscribe(&customer, &ctx.plan_id);

    // Cycle 1 (immediate)
    ctx.env.ledger().set_timestamp(1_000);
    ctx.client.charge_subscription(&sub_id);

    // Cycle 2 (after 1 interval)
    ctx.env.ledger().set_timestamp(1_000 + MONTHLY_INTERVAL);
    ctx.client.charge_subscription(&sub_id);

    // Cycle 3 (after 2 intervals)
    ctx.env.ledger().set_timestamp(1_000 + 2 * MONTHLY_INTERVAL);
    ctx.client.charge_subscription(&sub_id);

    let tok = token::TokenClient::new(&ctx.env, &ctx.token);
    assert_eq!(tok.balance(&customer), 10_000 - 3_000);

    let fee_per_charge = 1_000 * 500 / 10_000; // 50
    assert_eq!(tok.balance(&ctx.admin), fee_per_charge * 3);
    assert_eq!(
        tok.balance(&ctx.merchant_account_id),
        (1_000 - fee_per_charge) * 3
    );
}

// ---------------------------------------------------------------------------
// Test Case 8: Double cancellation fails
// ---------------------------------------------------------------------------
#[test]
#[should_panic(expected = "HostError: Error(Contract, #25)")]
fn test_double_cancellation_fails() {
    let ctx = setup_subscription_env();

    let customer = Address::generate(&ctx.env);
    let sub_id = ctx.client.subscribe(&customer, &ctx.plan_id);

    ctx.client.cancel_subscription(&customer, &sub_id);
    // Second cancellation should fail
    ctx.client.cancel_subscription(&customer, &sub_id);
}

// ---------------------------------------------------------------------------
// Test Case 9: Create plan with invalid parameters
// ---------------------------------------------------------------------------
#[test]
#[should_panic(expected = "HostError: Error(Contract, #7)")]
fn test_create_plan_zero_amount_fails() {
    let ctx = setup_subscription_env();
    let description = String::from_str(&ctx.env, "Bad Plan");
    ctx.client.create_subscription_plan(
        &ctx.merchant,
        &description,
        &ctx.token,
        &0,
        &MONTHLY_INTERVAL,
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #21)")]
fn test_create_plan_zero_interval_fails() {
    let ctx = setup_subscription_env();
    let description = String::from_str(&ctx.env, "Bad Plan");
    ctx.client
        .create_subscription_plan(&ctx.merchant, &description, &ctx.token, &1_000, &0);
}

// ---------------------------------------------------------------------------
// Test Case 10: Charge with zero-fee token
// ---------------------------------------------------------------------------
#[test]
fn test_charge_with_zero_fee() {
    let ctx = setup_subscription_env();

    // Set fee to 0
    ctx.client.set_fee(&ctx.admin, &ctx.token, &0);

    let customer = Address::generate(&ctx.env);
    fund_and_approve(&ctx, &customer, 5_000);

    let sub_id = ctx.client.subscribe(&customer, &ctx.plan_id);
    ctx.client.charge_subscription(&sub_id);

    let tok = token::TokenClient::new(&ctx.env, &ctx.token);
    assert_eq!(tok.balance(&ctx.merchant_account_id), 1_000);
    assert_eq!(tok.balance(&ctx.shade_id), 0);
    assert_eq!(tok.balance(&customer), 4_000);
}
