#![cfg(test)]

use crate::shade::{Shade, ShadeClient};
use crate::types::SubscriptionStatus;
use account::account::{MerchantAccount, MerchantAccountClient};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{Address, Env, String};

const MONTHLY_INTERVAL: u64 = 2_592_000;

struct EnrollCtx<'a> {
    env: Env,
    client: ShadeClient<'a>,
    merchant: Address,
    token: Address,
    plan_id: u64,
}

fn setup_enroll_env() -> EnrollCtx<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let shade_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &shade_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let token_admin = Address::generate(&env);
    let token_addr = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    client.add_accepted_token(&admin, &token_addr);
    client.set_fee(&admin, &token_addr, &500_i128);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let merchant_account_id = env.register(MerchantAccount, ());
    let merchant_account = MerchantAccountClient::new(&env, &merchant_account_id);
    merchant_account.initialize(&merchant, &shade_id, &1_u64);
    client.set_merchant_account(&merchant, &merchant_account_id);

    let plan_id = client.create_subscription_plan(
        &merchant,
        &String::from_str(&env, "Monthly Pro"),
        &token_addr,
        &1_000_i128,
        &MONTHLY_INTERVAL,
    );

    EnrollCtx { env, client, merchant, token: token_addr, plan_id }
}

// ---------------------------------------------------------------------------
// Valid enrollment
// ---------------------------------------------------------------------------

/// A new customer can enroll in an active plan; the returned subscription
/// has the correct plan_id, customer address, status, and start time.
#[test]
fn test_valid_enrollment_creates_active_subscription() {
    let ctx = setup_enroll_env();
    let customer = Address::generate(&ctx.env);

    ctx.env.ledger().set_timestamp(1_700_000_000);

    let sub_id = ctx.client.subscribe(&customer, &ctx.plan_id);
    let sub = ctx.client.get_subscription(&sub_id);

    assert_eq!(sub.plan_id, ctx.plan_id);
    assert_eq!(sub.customer, customer);
    assert_eq!(sub.status, SubscriptionStatus::Active);
    // start time must equal the ledger timestamp at enrollment
    assert_eq!(sub.date_created, 1_700_000_000);
    // no charge has occurred yet
    assert_eq!(sub.last_charged, 0);
}

/// Multiple customers can independently enroll in the same plan; each
/// receives a unique subscription ID.
#[test]
fn test_multiple_customers_can_enroll_same_plan() {
    let ctx = setup_enroll_env();

    let customer1 = Address::generate(&ctx.env);
    let customer2 = Address::generate(&ctx.env);
    let customer3 = Address::generate(&ctx.env);

    let sub_id1 = ctx.client.subscribe(&customer1, &ctx.plan_id);
    let sub_id2 = ctx.client.subscribe(&customer2, &ctx.plan_id);
    let sub_id3 = ctx.client.subscribe(&customer3, &ctx.plan_id);

    // All subscription IDs are distinct.
    assert_ne!(sub_id1, sub_id2);
    assert_ne!(sub_id2, sub_id3);
    assert_ne!(sub_id1, sub_id3);

    assert_eq!(ctx.client.get_subscription(&sub_id1).customer, customer1);
    assert_eq!(ctx.client.get_subscription(&sub_id2).customer, customer2);
    assert_eq!(ctx.client.get_subscription(&sub_id3).customer, customer3);

    // All subscriptions reference the same plan.
    assert_eq!(ctx.client.get_subscription(&sub_id1).plan_id, ctx.plan_id);
    assert_eq!(ctx.client.get_subscription(&sub_id2).plan_id, ctx.plan_id);
    assert_eq!(ctx.client.get_subscription(&sub_id3).plan_id, ctx.plan_id);
}

/// Enrollment preserves the exact ledger timestamp as the subscription
/// start time for billing-interval calculations.
#[test]
fn test_enrollment_stores_start_time() {
    let ctx = setup_enroll_env();
    ctx.env.ledger().set_timestamp(9_999_999);

    let customer = Address::generate(&ctx.env);
    let sub_id = ctx.client.subscribe(&customer, &ctx.plan_id);

    assert_eq!(ctx.client.get_subscription(&sub_id).date_created, 9_999_999);
}

// ---------------------------------------------------------------------------
// Invalid plan — non-existent
// ---------------------------------------------------------------------------

/// Attempting to subscribe to a plan that has never been created panics with
/// PlanNotFound (error code #22).
#[test]
#[should_panic(expected = "HostError: Error(Contract, #22)")]
fn test_subscribe_nonexistent_plan_panics() {
    let ctx = setup_enroll_env();
    let customer = Address::generate(&ctx.env);
    ctx.client.subscribe(&customer, &9999_u64);
}

// ---------------------------------------------------------------------------
// Invalid plan — deactivated
// ---------------------------------------------------------------------------

/// A deactivated plan must not accept new subscribers.
/// `subscribe` panics with PlanNotActive (error code #23).
#[test]
#[should_panic(expected = "HostError: Error(Contract, #23)")]
fn test_subscribe_deactivated_plan_panics() {
    let ctx = setup_enroll_env();

    // Merchant deactivates the plan.
    ctx.client.deactivate_plan(&ctx.merchant, &ctx.plan_id);

    let customer = Address::generate(&ctx.env);
    ctx.client.subscribe(&customer, &ctx.plan_id);
}

/// Customers already enrolled before deactivation keep their Active status.
#[test]
fn test_existing_subscribers_unaffected_by_plan_deactivation() {
    let ctx = setup_enroll_env();

    let customer = Address::generate(&ctx.env);
    let sub_id = ctx.client.subscribe(&customer, &ctx.plan_id);

    // Deactivate plan after enrollment.
    ctx.client.deactivate_plan(&ctx.merchant, &ctx.plan_id);

    // Pre-existing subscription remains active.
    let sub = ctx.client.get_subscription(&sub_id);
    assert_eq!(sub.status, SubscriptionStatus::Active);

    // The plan itself is now inactive.
    let plan = ctx.client.get_subscription_plan(&ctx.plan_id);
    assert!(!plan.active);
}

// ---------------------------------------------------------------------------
// Plan ID is stored correctly
// ---------------------------------------------------------------------------

/// The subscription's plan_id field must exactly match the ID used during
/// enrollment; this is the link used by the charge engine.
#[test]
fn test_enrollment_stores_correct_plan_id() {
    let ctx = setup_enroll_env();

    // Create a second plan to ensure we can distinguish plan IDs.
    let plan_id2 = ctx.client.create_subscription_plan(
        &ctx.merchant,
        &String::from_str(&ctx.env, "Annual Pro"),
        &ctx.token,
        &10_000_i128,
        &(MONTHLY_INTERVAL * 12),
    );

    let customer1 = Address::generate(&ctx.env);
    let customer2 = Address::generate(&ctx.env);

    let sub_id1 = ctx.client.subscribe(&customer1, &ctx.plan_id);
    let sub_id2 = ctx.client.subscribe(&customer2, &plan_id2);

    assert_eq!(ctx.client.get_subscription(&sub_id1).plan_id, ctx.plan_id);
    assert_eq!(ctx.client.get_subscription(&sub_id2).plan_id, plan_id2);
    // They must reference different plans.
    assert_ne!(ctx.client.get_subscription(&sub_id1).plan_id, plan_id2);
}
