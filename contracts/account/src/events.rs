use soroban_sdk::{contractevent, Address, Env};

#[contractevent]
pub struct AccountInitializedEvent {
    pub merchant: Address,
    pub merchant_id: u64,
    pub timestamp: u64,
}

pub fn publish_account_initialized_event(
    env: &Env,
    merchant: Address,
    merchant_id: u64,
    timestamp: u64,
) {
    AccountInitializedEvent {
        merchant,
        merchant_id,
        timestamp,
    }
    .publish(env);
}

#[contractevent(topics = ["account_restricted"])]
pub struct AccountRestrictedEvent {
    pub status: bool,
    pub timestamp: u64,
}

pub fn publish_account_restricted_event(env: &Env, status: bool, timestamp: u64) {
    AccountRestrictedEvent { status, timestamp }.publish(env);
}

#[contractevent(data_format = "single-value")]
pub struct AccountVerifiedEvent {
    pub timestamp: u64,
}

/// Alias used by tests.
pub type AccountVerified = AccountVerifiedEvent;

pub fn publish_account_verified_event(env: &Env, timestamp: u64) {
    AccountVerifiedEvent { timestamp }.publish(env);
}

#[contractevent]
pub struct RefundProcessedEvent {
    pub token: Address,
    pub amount: i128,
    pub recipient: Address,
    pub timestamp: u64,
}

pub fn publish_refund_processed_event(
    env: &Env,
    token: Address,
    amount: i128,
    to: Address,
    timestamp: u64,
) {
    RefundProcessedEvent {
        token,
        amount,
        recipient: to,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct TokenAddedEvent {
    pub token: Address,
    pub timestamp: u64,
}

pub fn publish_token_added_event(env: &Env, token: Address, timestamp: u64) {
    TokenAddedEvent { token, timestamp }.publish(env);
}

#[contractevent]
pub struct WithdrawalToEvent {
    pub token: Address,
    pub merchant: Address,
    pub recipient: Address,
    pub amount: i128,
    pub timestamp: u64,
}

pub fn publish_withdrawal_to_event(
    env: &Env,
    token: Address,
    merchant: Address,
    recipient: Address,
    amount: i128,
    timestamp: u64,
) {
    WithdrawalToEvent {
        token,
        merchant,
        recipient,
        amount,
        timestamp,
    }
    .publish(env);
}
