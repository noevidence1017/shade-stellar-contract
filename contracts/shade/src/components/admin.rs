use crate::components::{core, reentrancy};
use crate::errors::ContractError;
use crate::events;
use crate::types::DataKey;
use soroban_sdk::{panic_with_error, token, Address, Env, Vec};

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

pub fn calculate_fee(env: &Env, token: &Address, amount: i128) -> i128 {
    let fee_bps: i128 = get_fee(env, token);
    if fee_bps == 0 {
        return 0;
    }
    (amount * fee_bps) / 10_000i128
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
