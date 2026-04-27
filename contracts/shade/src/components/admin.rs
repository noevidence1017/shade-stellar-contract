use crate::components::{core, reentrancy};
use crate::errors::ContractError;
use crate::events;
use crate::types::{
    DataKey, MerchantAnalytics, MerchantAnalyticsSummary, OracleConfig, PendingFee, TokenAnalytics,
};
use soroban_sdk::{panic_with_error, token, Address, Env, Vec};

pub const FEE_UPDATE_DELAY: u64 = 172_800; // 48 hours in seconds

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

pub fn set_platform_account(env: &Env, admin: &Address, account: &Address) {
    reentrancy::enter(env);
    core::assert_admin(env, admin);
    env.storage()
        .persistent()
        .set(&DataKey::PlatformAccount, account);
    events::publish_platform_account_set_event(
        env,
        admin.clone(),
        account.clone(),
        env.ledger().timestamp(),
    );
    reentrancy::exit(env);
}

pub fn get_platform_account(env: &Env) -> Address {
    env.storage()
        .persistent()
        .get(&DataKey::PlatformAccount)
        .unwrap_or_else(|| core::get_admin(env))
}

pub fn set_token_oracle(env: &Env, admin: &Address, token: &Address, oracle: &OracleConfig) {
    reentrancy::enter(env);
    core::assert_admin(env, admin);

    if !is_accepted_token(env, token) {
        panic_with_error!(env, ContractError::TokenNotAccepted);
    }

    env.storage()
        .persistent()
        .set(&DataKey::TokenOracle(token.clone()), oracle);

    events::publish_token_oracle_set_event(
        env,
        admin.clone(),
        token.clone(),
        oracle.contract.clone(),
        env.ledger().timestamp(),
    );
    reentrancy::exit(env);
}

pub fn get_token_oracle(env: &Env, token: &Address) -> OracleConfig {
    env.storage()
        .persistent()
        .get(&DataKey::TokenOracle(token.clone()))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::OracleNotConfigured))
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
    get_merchant_analytics(env, merchant, token).total_volume
}

pub fn get_merchant_analytics(env: &Env, merchant: &Address, token: &Address) -> MerchantAnalytics {
    env.storage()
        .persistent()
        .get(&DataKey::MerchantAnalytics(merchant.clone(), token.clone()))
        .unwrap_or(MerchantAnalytics {
            merchant: merchant.clone(),
            token: token.clone(),
            total_volume: env
                .storage()
                .persistent()
                .get(&DataKey::MerchantVolume(merchant.clone(), token.clone()))
                .unwrap_or(0),
            total_fees: 0,
            transaction_count: 0,
            last_updated: 0,
        })
}

pub fn get_merchant_analytics_summary(env: &Env, merchant: &Address) -> MerchantAnalyticsSummary {
    env.storage()
        .persistent()
        .get(&DataKey::MerchantAnalyticsSummary(merchant.clone()))
        .unwrap_or(MerchantAnalyticsSummary {
            merchant: merchant.clone(),
            total_volume: 0,
            total_fees: 0,
            transaction_count: 0,
            last_updated: 0,
        })
}

pub fn record_merchant_payment(
    env: &Env,
    merchant: &Address,
    token: &Address,
    volume_amount: i128,
    fee_amount: i128,
) {
    let mut analytics = get_merchant_analytics(env, merchant, token);
    analytics.total_volume += volume_amount;
    analytics.total_fees += fee_amount;
    analytics.transaction_count += 1;
    analytics.last_updated = env.ledger().timestamp();

    env.storage().persistent().set(
        &DataKey::MerchantAnalytics(merchant.clone(), token.clone()),
        &analytics,
    );
    env.storage().persistent().set(
        &DataKey::MerchantVolume(merchant.clone(), token.clone()),
        &analytics.total_volume,
    );

    let mut summary = get_merchant_analytics_summary(env, merchant);
    summary.total_volume += volume_amount;
    summary.total_fees += fee_amount;
    summary.transaction_count += 1;
    summary.last_updated = analytics.last_updated;

    env.storage().persistent().set(
        &DataKey::MerchantAnalyticsSummary(merchant.clone()),
        &summary,
    );

    // Update global token analytics
    record_token_payment(env, token, volume_amount, fee_amount);
}

pub fn get_token_analytics(env: &Env, token: &Address) -> TokenAnalytics {
    env.storage()
        .persistent()
        .get(&DataKey::TokenAnalytics(token.clone()))
        .unwrap_or(TokenAnalytics {
            token: token.clone(),
            total_volume: 0,
            total_fees: 0,
            transaction_count: 0,
            unique_merchants: 0,
            last_updated: 0,
        })
}

pub fn get_token_volume(env: &Env, token: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::TokenVolume(token.clone()))
        .unwrap_or(0)
}

fn record_token_payment(env: &Env, token: &Address, volume_amount: i128, fee_amount: i128) {
    let mut analytics = get_token_analytics(env, token);
    
    // Check if this is a new merchant for this token
    let current_volume = get_token_volume(env, token);
    let is_new_merchant = current_volume == 0;
    
    analytics.total_volume += volume_amount;
    analytics.total_fees += fee_amount;
    analytics.transaction_count += 1;
    if is_new_merchant {
        analytics.unique_merchants += 1;
    }
    analytics.last_updated = env.ledger().timestamp();

    env.storage().persistent().set(
        &DataKey::TokenAnalytics(token.clone()),
        &analytics,
    );
    
    env.storage().persistent().set(
        &DataKey::TokenVolume(token.clone()),
        &analytics.total_volume,
    );
}

pub fn get_token_dominance_metrics(env: &Env, tokens: &Vec<Address>) -> Vec<(Address, i128)> {
    let mut token_volumes: Vec<(Address, i128)> = Vec::new(env);
    let mut total_volume: i128 = 0;
    
    // Calculate total volume across all tokens
    for token in tokens.iter() {
        let volume = get_token_volume(env, token);
        total_volume += volume;
        token_volumes.push_back((token.clone(), volume));
    }
    
    // Sort by volume in descending order
    token_volumes.sort_by(|a, b| b.1.cmp(&a.1));
    
    token_volumes
}

pub fn get_top_tokens_by_volume(env: &Env, limit: u32) -> Vec<(Address, i128)> {
    let accepted_tokens = crate::components::admin::get_accepted_tokens(env);
    let mut all_metrics = get_token_dominance_metrics(env, &accepted_tokens);
    
    // Truncate to specified limit
    while all_metrics.len() > limit {
        all_metrics.pop_back();
    }
    
    all_metrics
}

pub fn get_token_market_share(env: &Env, token: &Address) -> i128 {
    let token_volume = get_token_volume(env, token);
    if token_volume == 0 {
        return 0;
    }
    
    let accepted_tokens = crate::components::admin::get_accepted_tokens(env);
    let mut total_volume: i128 = 0;
    
    for t in accepted_tokens.iter() {
        total_volume += get_token_volume(env, t);
    }
    
    if total_volume == 0 {
        return 0;
    }
    
    // Return market share as basis points (10000 = 100%)
    (token_volume * 10000) / total_volume
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
