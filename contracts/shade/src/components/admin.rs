use crate::components::{core, reentrancy};
use crate::errors::ContractError;
use crate::events;
use crate::types::{DataKey, PendingFee};
use soroban_sdk::{panic_with_error, token, Address, Env, Vec};

pub const FEE_UPDATE_DELAY: u64 = 172_800; // 48 hours in seconds
pub const DAY_IN_SECONDS: u64 = 86400;
pub const WEEK_IN_SECONDS: u64 = 604800;

// TODO: create the functionality for withdrawing revenue by admin.

pub fn add_accepted_token(env: &Env, admin: &Address, token: &Address) {
    reentrancy::enter(env);
    core::assert_admin(env, admin);

    let mut accepted_tokens = get_accepted_tokens(env);
    if !contains_token(&accepted_tokens, token) {
        let _ = token::Client::new(env, token).symbol();
        accepted_tokens.push_back(token.clone());
        env.storage()
            .persistent()
            .set(&DataKey::AcceptedTokens, &accepted_tokens);
        events::publish_token_added_event(env, token.clone(), env.ledger().timestamp());
    }
    reentrancy::exit(env);
}

pub fn add_accepted_tokens(env: &Env, admin: &Address, tokens: &Vec<Address>) {
    reentrancy::enter(env);
    core::assert_admin(env, admin);

    let mut accepted_tokens = get_accepted_tokens(env);
    let mut changed = false;
    let timestamp = env.ledger().timestamp();

    for token in tokens.iter() {
        if !contains_token(&accepted_tokens, &token) {
            let _ = token::Client::new(env, &token).symbol();
            accepted_tokens.push_back(token.clone());
            events::publish_token_added_event(env, token.clone(), timestamp);
            changed = true;
        }
    }

    if changed {
        env.storage()
            .persistent()
            .set(&DataKey::AcceptedTokens, &accepted_tokens);
    }
    reentrancy::exit(env);
}

pub fn remove_accepted_token(env: &Env, admin: &Address, token: &Address) {
    reentrancy::enter(env);
    core::assert_admin(env, admin);

    let accepted_tokens = get_accepted_tokens(env);
    let mut updated_tokens = Vec::new(env);
    let mut removed = false;

    for accepted_token in accepted_tokens.iter() {
        if accepted_token == *token {
            removed = true;
        } else {
            updated_tokens.push_back(accepted_token);
        }
    }

    if removed {
        env.storage()
            .persistent()
            .set(&DataKey::AcceptedTokens, &updated_tokens);
        events::publish_token_removed_event(env, token.clone(), env.ledger().timestamp());
    }
    reentrancy::exit(env);
}

pub fn is_accepted_token(env: &Env, token: &Address) -> bool {
    contains_token(&get_accepted_tokens(env), token)
}

fn contains_token(accepted_tokens: &Vec<Address>, token: &Address) -> bool {
    for accepted_token in accepted_tokens.iter() {
        if accepted_token == *token {
            return true;
        }
    }
    false
}

pub fn set_account_wasm_hash(env: &Env, admin: &Address, wasm_hash: &soroban_sdk::BytesN<32>) {
    reentrancy::enter(env);
    core::assert_admin(env, admin);
    env.storage()
        .persistent()
        .set(&DataKey::AccountWasmHash, wasm_hash);
    events::publish_account_wasm_hash_set_event(
        env,
        admin.clone(),
        wasm_hash.clone(),
        env.ledger().timestamp(),
    );
    reentrancy::exit(env);
}

pub fn set_fee(env: &Env, admin: &Address, token: &Address, fee: i128) {
    reentrancy::enter(env);
    core::assert_admin(env, admin);

    if !is_accepted_token(env, token) {
        panic_with_error!(env, ContractError::TokenNotAccepted);
    }

    env.storage()
        .persistent()
        .set(&DataKey::TokenFee(token.clone()), &fee);

    events::publish_fee_set_event(
        env,
        admin.clone(),
        token.clone(),
        fee,
        env.ledger().timestamp(),
    );
    reentrancy::exit(env);
}

pub fn get_fee(env: &Env, token: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::TokenFee(token.clone()))
        .unwrap_or(0)
}

pub fn calculate_fee(env: &Env, merchant: &Address, token: &Address, amount: i128) -> i128 {
    let fee_bps: i128 = get_fee(env, token);
    if fee_bps == 0 {
        return 0;
    }

    let volume = get_merchant_volume(env, merchant, token);
    let discounted_bps = apply_volume_discount(fee_bps, volume);

    (amount * discounted_bps) / 10_000i128
}

pub fn get_merchant_volume(env: &Env, merchant: &Address, token: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::MerchantVolume(merchant.clone(), token.clone()))
        .unwrap_or(0)
}

pub fn increment_merchant_volume(env: &Env, merchant: &Address, token: &Address, amount: i128) {
    let current = get_merchant_volume(env, merchant, token);
    env.storage().persistent().set(
        &DataKey::MerchantVolume(merchant.clone(), token.clone()),
        &(current + amount),
    );

    // Time-bucketed stats
    let now = env.ledger().timestamp();
    let day_bin = (now / DAY_IN_SECONDS) * DAY_IN_SECONDS;
    let week_bin = (now / WEEK_IN_SECONDS) * WEEK_IN_SECONDS;

    // Global stats
    let daily_key = DataKey::DailyVolume(token.clone(), day_bin);
    let weekly_key = DataKey::WeeklyVolume(token.clone(), week_bin);

    let current_daily: i128 = env.storage().persistent().get(&daily_key).unwrap_or(0);
    let current_weekly: i128 = env.storage().persistent().get(&weekly_key).unwrap_or(0);

    env.storage()
        .persistent()
        .set(&daily_key, &(current_daily + amount));
    env.storage()
        .persistent()
        .set(&weekly_key, &(current_weekly + amount));

    // Merchant time-bucketed stats
    let m_daily_key = DataKey::MerchantDailyVolume(merchant.clone(), token.clone(), day_bin);
    let m_weekly_key = DataKey::MerchantWeeklyVolume(merchant.clone(), token.clone(), week_bin);

    let current_m_daily: i128 = env.storage().persistent().get(&m_daily_key).unwrap_or(0);
    let current_m_weekly: i128 = env.storage().persistent().get(&m_weekly_key).unwrap_or(0);

    env.storage()
        .persistent()
        .set(&m_daily_key, &(current_m_daily + amount));
    env.storage()
        .persistent()
        .set(&m_weekly_key, &(current_m_weekly + amount));
}

pub fn get_daily_volume(env: &Env, token: &Address) -> i128 {
    let now = env.ledger().timestamp();
    let day_bin = (now / DAY_IN_SECONDS) * DAY_IN_SECONDS;
    env.storage()
        .persistent()
        .get(&DataKey::DailyVolume(token.clone(), day_bin))
        .unwrap_or(0)
}

pub fn get_weekly_volume(env: &Env, token: &Address) -> i128 {
    let now = env.ledger().timestamp();
    let week_bin = (now / WEEK_IN_SECONDS) * WEEK_IN_SECONDS;
    env.storage()
        .persistent()
        .get(&DataKey::WeeklyVolume(token.clone(), week_bin))
        .unwrap_or(0)
}

pub fn get_merchant_daily_volume(env: &Env, merchant: &Address, token: &Address) -> i128 {
    let now = env.ledger().timestamp();
    let day_bin = (now / DAY_IN_SECONDS) * DAY_IN_SECONDS;
    env.storage()
        .persistent()
        .get(&DataKey::MerchantDailyVolume(
            merchant.clone(),
            token.clone(),
            day_bin,
        ))
        .unwrap_or(0)
}

pub fn get_merchant_weekly_volume(env: &Env, merchant: &Address, token: &Address) -> i128 {
    let now = env.ledger().timestamp();
    let week_bin = (now / WEEK_IN_SECONDS) * WEEK_IN_SECONDS;
    env.storage()
        .persistent()
        .get(&DataKey::MerchantWeeklyVolume(
            merchant.clone(),
            token.clone(),
            week_bin,
        ))
        .unwrap_or(0)
}

fn apply_volume_discount(fee_bps: i128, volume: i128) -> i128 {
    let discount_percentage = if volume >= 200_000 {
        50 // 50% discount
    } else if volume >= 50_000 {
        25 // 25% discount
    } else if volume >= 10_000 {
        10 // 10% discount
    } else {
        0
    };

    if discount_percentage == 0 {
        fee_bps
    } else {
        (fee_bps * (100 - discount_percentage)) / 100
    }
}

pub fn propose_fee(env: &Env, admin: &Address, token: &Address, fee: i128) {
    reentrancy::enter(env);
    core::assert_admin(env, admin);

    if !is_accepted_token(env, token) {
        panic_with_error!(env, ContractError::TokenNotAccepted);
    }

    let pending = PendingFee {
        token: token.clone(),
        fee,
        proposed_at: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&DataKey::PendingTokenFee(token.clone()), &pending);

    events::publish_fee_proposed_event(
        env,
        admin.clone(),
        token.clone(),
        fee,
        env.ledger().timestamp(),
    );
    reentrancy::exit(env);
}

pub fn execute_fee(env: &Env, admin: &Address, token: &Address) {
    reentrancy::enter(env);
    core::assert_admin(env, admin);

    let pending: PendingFee = env
        .storage()
        .persistent()
        .get(&DataKey::PendingTokenFee(token.clone()))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::NoPendingFeeUpdate));

    let elapsed = env.ledger().timestamp() - pending.proposed_at;
    if elapsed < FEE_UPDATE_DELAY {
        panic_with_error!(env, ContractError::FeeUpdateTooEarly);
    }

    env.storage()
        .persistent()
        .set(&DataKey::TokenFee(token.clone()), &pending.fee);

    env.storage()
        .persistent()
        .remove(&DataKey::PendingTokenFee(token.clone()));

    events::publish_fee_set_event(
        env,
        admin.clone(),
        token.clone(),
        pending.fee,
        env.ledger().timestamp(),
    );
    reentrancy::exit(env);
}

pub fn get_pending_fee(env: &Env, token: &Address) -> PendingFee {
    env.storage()
        .persistent()
        .get(&DataKey::PendingTokenFee(token.clone()))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::NoPendingFeeUpdate))
}

pub fn propose_admin_transfer(env: &Env, admin: &Address, new_admin: &Address) {
    core::assert_admin(env, admin);
    env.storage()
        .persistent()
        .set(&DataKey::PendingAdmin, new_admin);
    events::publish_admin_transfer_proposed_event(
        env,
        admin.clone(),
        new_admin.clone(),
        env.ledger().timestamp(),
    );
}

pub fn accept_admin_transfer(env: &Env, new_admin: &Address) {
    new_admin.require_auth();
    let pending: Address = env
        .storage()
        .persistent()
        .get(&DataKey::PendingAdmin)
        .unwrap_or_else(|| panic_with_error!(env, ContractError::NotAuthorized));

    if *new_admin != pending {
        panic_with_error!(env, ContractError::NotAuthorized);
    }

    let old_admin: Address = core::get_admin(env);
    env.storage().persistent().set(&DataKey::Admin, new_admin);
    env.storage().persistent().remove(&DataKey::PendingAdmin);
    events::publish_admin_transfer_accepted_event(
        env,
        old_admin,
        new_admin.clone(),
        env.ledger().timestamp(),
    );
}

fn get_accepted_tokens(env: &Env) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::AcceptedTokens)
        .unwrap_or_else(|| Vec::new(env))
}
