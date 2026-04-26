use crate::components::{admin, history, merchant};
use crate::errors::ContractError;
use crate::events;
use crate::types::{DataKey, Subscription, SubscriptionPlan, SubscriptionStatus, Transaction, TransactionType};
use soroban_sdk::{panic_with_error, token, Address, Env, String};

// TODO: create a functionality for bulk subscription plan charging
// TODO: create a functionality for charging all the subscription in a plan

fn get_plan_count(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::PlanCount)
        .unwrap_or(0)
}

fn get_subscription_count(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::SubscriptionCount)
        .unwrap_or(0)
}

fn get_merchant_id(env: &Env, merchant: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::MerchantId(merchant.clone()))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::MerchantNotFound))
}

pub fn create_subscription_plan(
    env: &Env,
    merchant: Address,
    description: String,
    token: Address,
    amount: i128,
    interval: u64,
) -> u64 {
    merchant.require_auth();
    if amount <= 0 {
        panic_with_error!(env, ContractError::InvalidAmount);
    }
    if interval == 0 {
        panic_with_error!(env, ContractError::InvalidInterval);
    }
    if !admin::is_accepted_token(env, &token) {
        panic_with_error!(env, ContractError::TokenNotAccepted);
    }

    let fee_amount = admin::calculate_fee(env, &merchant, &token, amount);
    if amount < fee_amount {
        panic_with_error!(env, ContractError::InvalidAmount);
    }

    let merchant_id = get_merchant_id(env, &merchant);
    let plan_id = get_plan_count(env) + 1;
    env.storage()
        .persistent()
        .set(&DataKey::PlanCount, &plan_id);

    let plan = SubscriptionPlan {
        id: plan_id,
        merchant_id,
        merchant: merchant.clone(),
        description,
        token: token.clone(),
        amount,
        interval,
        active: true,
    };
    env.storage()
        .persistent()
        .set(&DataKey::SubscriptionPlan(plan_id), &plan);

    events::publish_subscription_plan_created_event(
        env,
        plan_id,
        merchant,
        token,
        amount,
        interval,
        env.ledger().timestamp(),
    );
    plan_id
}

pub fn get_subscription_plan(env: &Env, plan_id: u64) -> SubscriptionPlan {
    env.storage()
        .persistent()
        .get(&DataKey::SubscriptionPlan(plan_id))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::PlanNotFound))
}

pub fn subscribe(env: &Env, customer: Address, plan_id: u64) -> u64 {
    // TODO: determine if a customer is allowed to subscribe more than once to the same plan
    // and if so, create a storage for saving the subcription ids of a plan in a list
    let plan = get_subscription_plan(env, plan_id);
    if !plan.active {
        panic_with_error!(env, ContractError::PlanNotActive);
    }

    let sub_id = get_subscription_count(env) + 1;
    env.storage()
        .persistent()
        .set(&DataKey::SubscriptionCount, &sub_id);

    let now = env.ledger().timestamp();
    let sub = Subscription {
        id: sub_id,
        plan_id,
        customer: customer.clone(),
        merchant_id: plan.merchant_id,
        status: SubscriptionStatus::Active,
        date_created: now,
        last_charged: 0,
    };
    env.storage()
        .persistent()
        .set(&DataKey::Subscription(sub_id), &sub);

    events::publish_subscribed_event(env, sub_id, plan_id, customer, now);
    sub_id
}

pub fn get_subscription(env: &Env, subscription_id: u64) -> Subscription {
    env.storage()
        .persistent()
        .get(&DataKey::Subscription(subscription_id))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::SubscriptionNotFound))
}

pub fn charge_subscription(env: &Env, subscription_id: u64) {
    let mut sub = get_subscription(env, subscription_id);
    if sub.status != SubscriptionStatus::Active {
        panic_with_error!(env, ContractError::SubscriptionNotActive);
    }

    let plan = get_subscription_plan(env, sub.plan_id);
    let now = env.ledger().timestamp();
    if sub.last_charged > 0 && now < sub.last_charged.saturating_add(plan.interval) {
        panic_with_error!(env, ContractError::ChargeTooEarly);
    }

    let fee = admin::calculate_fee(env, &plan.merchant, &plan.token, plan.amount);
    let merchant_amount = plan.amount - fee;

    let token_client = token::TokenClient::new(env, &plan.token);
    let merchant_account = merchant::get_merchant_account(env, plan.merchant_id);
    let platform_account = admin::get_platform_account(env);
    let spender = env.current_contract_address();

    token_client.transfer_from(&spender, &sub.customer, &merchant_account, &merchant_amount);
    if fee > 0 {
        token_client.transfer_from(&spender, &sub.customer, &platform_account, &fee);
    }
    admin::record_merchant_payment(env, &plan.merchant, &plan.token, plan.amount, fee);

    sub.last_charged = now;
    env.storage()
        .persistent()
        .set(&DataKey::Subscription(subscription_id), &sub);

    events::publish_subscription_charged_event(
        env,
        subscription_id,
        plan.id,
        sub.customer.clone(),
        plan.merchant.clone(),
        plan.amount,
        fee,
        plan.token.clone(),
        now,
    );

    let transaction = Transaction {
        transaction_type: TransactionType::SubscriptionCharge,
        ref_id: subscription_id,
        amount: plan.amount,
        token: plan.token.clone(),
        description: plan.description.clone(),
        date: now,
        merchant_id: plan.merchant_id,
    };
    history::record_transaction(env, &sub.customer, transaction);
}

pub fn cancel_subscription(env: &Env, caller: Address, subscription_id: u64) {
    caller.require_auth();
    let mut sub = get_subscription(env, subscription_id);
    if sub.status != SubscriptionStatus::Active {
        panic_with_error!(env, ContractError::SubscriptionNotActive);
    }

    let plan = get_subscription_plan(env, sub.plan_id);
    let is_customer = sub.customer == caller;
    let is_merchant = plan.merchant == caller;
    if !is_customer && !is_merchant {
        panic_with_error!(env, ContractError::NotAuthorized);
    }

    sub.status = SubscriptionStatus::Cancelled;
    env.storage()
        .persistent()
        .set(&DataKey::Subscription(subscription_id), &sub);

    events::publish_subscription_cancelled_event(
        env,
        subscription_id,
        caller,
        env.ledger().timestamp(),
    );
}

pub fn deactivate_plan(env: &Env, caller: Address, plan_id: u64) {
    caller.require_auth();
    let mut plan = get_subscription_plan(env, plan_id);
    if plan.merchant != caller {
        panic_with_error!(env, ContractError::NotAuthorized);
    }
    plan.active = false;
    env.storage()
        .persistent()
        .set(&DataKey::SubscriptionPlan(plan_id), &plan);
    events::publish_plan_deactivated_event(env, plan_id, caller, env.ledger().timestamp());
}
