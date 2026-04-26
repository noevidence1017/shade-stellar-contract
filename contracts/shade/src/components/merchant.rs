use crate::components::access_control;
use crate::components::admin as admin_component;
use crate::components::core as core_component;
use crate::errors::ContractError;
use crate::events;
use crate::types::{DataKey, Merchant, MerchantFilter, Role};
use soroban_sdk::{contractclient, panic_with_error, Address, BytesN, Env, String, Vec};

#[contractclient(name = "MerchantAccountClient")]
pub trait MerchantAccountContract {
    fn restrict_account(env: Env, status: bool);
}

pub fn register_merchant(env: &Env, merchant: &Address) {
    merchant.require_auth();

    if env
        .storage()
        .persistent()
        .has(&DataKey::MerchantId(merchant.clone()))
    {
        panic_with_error!(env, ContractError::MerchantAlreadyRegistered);
    }

    let merchant_count: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::MerchantCount)
        .unwrap_or(0);

    let new_id = merchant_count + 1;

    let merchant_data = Merchant {
        id: new_id,
        address: merchant.clone(),
        active: true,
        verified: false,
        date_registered: env.ledger().timestamp(),
        account: merchant.clone(),
        webhook: String::from_str(env, ""),
    };

    env.storage()
        .persistent()
        .set(&DataKey::Merchant(new_id), &merchant_data);
    env.storage()
        .persistent()
        .set(&DataKey::MerchantId(merchant.clone()), &new_id);
    env.storage()
        .persistent()
        .set(&DataKey::MerchantCount, &new_id);

    events::publish_merchant_registered_event(
        env,
        merchant.clone(),
        new_id,
        env.ledger().timestamp(),
    );
}

pub fn get_merchant(env: &Env, merchant_id: u64) -> Merchant {
    if merchant_id == 0 {
        panic_with_error!(env, ContractError::MerchantNotFound);
    }

    let merchant_count: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::MerchantCount)
        .unwrap_or(0);

    if merchant_id > merchant_count {
        panic_with_error!(env, ContractError::MerchantNotFound);
    }

    env.storage()
        .persistent()
        .get(&DataKey::Merchant(merchant_id))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::MerchantNotFound))
}

pub fn get_merchant_by_address(env: &Env, merchant: &Address) -> Merchant {
    let merchant_id = env
        .storage()
        .persistent()
        .get(&DataKey::MerchantId(merchant.clone()))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::MerchantNotFound));
    get_merchant(env, merchant_id)
}

pub fn get_merchant_id(env: &Env, merchant: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::MerchantId(merchant.clone()))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::MerchantNotFound))
}

pub fn is_merchant(env: &Env, merchant: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::MerchantId(merchant.clone()))
}

pub fn set_merchant_status(env: &Env, admin: &Address, merchant_id: u64, status: bool) {
    core_component::assert_admin(env, admin);

    if merchant_id == 0 {
        panic_with_error!(env, ContractError::MerchantNotFound);
    }

    let merchant_count: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::MerchantCount)
        .unwrap_or(0);

    if merchant_id > merchant_count {
        panic_with_error!(env, ContractError::MerchantNotFound);
    }

    let mut merchant: Merchant = env
        .storage()
        .persistent()
        .get(&DataKey::Merchant(merchant_id))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::MerchantNotFound));

    merchant.active = status;

    env.storage()
        .persistent()
        .set(&DataKey::Merchant(merchant_id), &merchant);

    events::publish_merchant_status_changed_event(
        env,
        merchant_id,
        status,
        env.ledger().timestamp(),
    );
}

pub fn is_merchant_active(env: &Env, merchant_id: u64) -> bool {
    if merchant_id == 0 {
        panic_with_error!(env, ContractError::MerchantNotFound);
    }

    let merchant_count: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::MerchantCount)
        .unwrap_or(0);

    if merchant_id > merchant_count {
        panic_with_error!(env, ContractError::MerchantNotFound);
    }

    let merchant: Merchant = env
        .storage()
        .persistent()
        .get(&DataKey::Merchant(merchant_id))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::MerchantNotFound));

    merchant.active
}

pub fn verify_merchant(env: &Env, admin: &Address, merchant_id: u64, status: bool) {
    core_component::assert_admin(env, admin);

    let mut merchant_data = get_merchant(env, merchant_id);
    merchant_data.verified = status;

    env.storage()
        .persistent()
        .set(&DataKey::Merchant(merchant_id), &merchant_data);

    events::publish_merchant_verified_event(env, merchant_id, status, env.ledger().timestamp());
}

pub fn is_merchant_verified(env: &Env, merchant_id: u64) -> bool {
    let merchant_data = get_merchant(env, merchant_id);
    merchant_data.verified
}

pub fn set_merchant_key(env: &Env, merchant: &Address, key: &BytesN<32>) {
    merchant.require_auth();

    if !is_merchant(env, merchant) {
        panic_with_error!(env, ContractError::MerchantNotFound);
    }

    env.storage()
        .persistent()
        .set(&DataKey::MerchantKey(merchant.clone()), key);

    events::publish_merchant_key_set_event(
        env,
        merchant.clone(),
        key.clone(),
        env.ledger().timestamp(),
    );
}

pub fn get_merchant_key(env: &Env, merchant: &Address) -> BytesN<32> {
    env.storage()
        .persistent()
        .get(&DataKey::MerchantKey(merchant.clone()))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::MerchantKeyNotFound))
}

pub fn get_merchants(env: &Env, filter: MerchantFilter) -> Vec<Merchant> {
    let merchant_count: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::MerchantCount)
        .unwrap_or(0);

    let mut merchants: Vec<Merchant> = Vec::new(env);

    for i in 1..=merchant_count {
        if let Some(merchant) = env
            .storage()
            .persistent()
            .get::<_, Merchant>(&DataKey::Merchant(i))
        {
            let mut matches = true;

            if let Some(active) = filter.is_active {
                if merchant.active != active {
                    matches = false;
                }
            }

            if let Some(verified) = filter.is_verified {
                if merchant.verified != verified {
                    matches = false;
                }
            }

            if matches {
                merchants.push_back(merchant);
            }
        }
    }

    merchants
}

pub fn restrict_merchant_account(
    env: &Env,
    caller: &Address,
    merchant_address: &Address,
    status: bool,
) {
    caller.require_auth();

    if !access_control::has_role(env, caller, Role::Admin)
        && !access_control::has_role(env, caller, Role::Manager)
    {
        panic_with_error!(env, ContractError::NotAuthorized);
    }

    let merchant_id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::MerchantId(merchant_address.clone()))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::MerchantNotFound));

    let account_address: Address = env
        .storage()
        .persistent()
        .get(&DataKey::MerchantAccount(merchant_id))
        .unwrap_or_else(|| merchant_address.clone());

    let client = MerchantAccountClient::new(env, &account_address);
    client.restrict_account(&status);

    events::publish_account_restricted_event(
        env,
        merchant_address.clone(),
        status,
        caller.clone(),
        env.ledger().timestamp(),
    );
}

pub fn set_merchant_account(env: &Env, merchant: &Address, account: &Address) {
    merchant.require_auth();

    if !is_merchant(env, merchant) {
        panic_with_error!(env, ContractError::MerchantNotFound);
    }

    let merchant_id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::MerchantId(merchant.clone()))
        .unwrap();

    env.storage()
        .persistent()
        .set(&DataKey::MerchantAccount(merchant_id), account);
}

pub fn get_merchant_account(env: &Env, merchant_id: u64) -> Address {
    env.storage()
        .persistent()
        .get(&DataKey::MerchantAccount(merchant_id))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::MerchantAccountNotSet))
}

pub fn set_merchant_accepted_tokens(env: &Env, merchant: &Address, tokens: &Vec<Address>) {
    merchant.require_auth();

    if !is_merchant(env, merchant) {
        panic_with_error!(env, ContractError::MerchantNotFound);
    }

    let merchant_id = get_merchant_id(env, merchant);
    if !is_merchant_active(env, merchant_id) {
        panic_with_error!(env, ContractError::MerchantNotActive);
    }

    // Deduplicate and verify all tokens are globally accepted
    let mut deduped: Vec<Address> = Vec::new(env);
    for token in tokens.iter() {
        if !admin_component::is_accepted_token(env, &token) {
            panic_with_error!(env, ContractError::TokenNotAccepted);
        }
        // Only add if not already present
        let mut found = false;
        for existing in deduped.iter() {
            if existing == token {
                found = true;
                break;
            }
        }
        if !found {
            deduped.push_back(token.clone());
        }
    }

    env.storage()
        .persistent()
        .set(&DataKey::MerchantTokens(merchant.clone()), &deduped);

    events::publish_merchant_tokens_set_event(
        env,
        merchant.clone(),
        deduped,
        env.ledger().timestamp(),
    );
}

pub fn remove_merchant_accepted_token(env: &Env, merchant: &Address, token: &Address) {
    merchant.require_auth();

    if !is_merchant(env, merchant) {
        panic_with_error!(env, ContractError::MerchantNotFound);
    }

    let merchant_id = get_merchant_id(env, merchant);
    if !is_merchant_active(env, merchant_id) {
        panic_with_error!(env, ContractError::MerchantNotActive);
    }

    let merchant_tokens = get_merchant_accepted_tokens(env, merchant);
    let mut updated: Vec<Address> = Vec::new(env);
    let mut found = false;

    for t in merchant_tokens.iter() {
        if t == *token {
            found = true;
        } else {
            updated.push_back(t);
        }
    }

    if !found {
        panic_with_error!(env, ContractError::TokenNotAccepted);
    }

    env.storage()
        .persistent()
        .set(&DataKey::MerchantTokens(merchant.clone()), &updated);

    events::publish_merchant_token_removed_event(
        env,
        merchant.clone(),
        token.clone(),
        env.ledger().timestamp(),
    );
}

pub fn get_merchant_accepted_tokens(env: &Env, merchant: &Address) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::MerchantTokens(merchant.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn set_merchant_webhook(env: &Env, merchant: &Address, webhook: &String) {
    merchant.require_auth();

    if !is_merchant(env, merchant) {
        panic_with_error!(env, ContractError::MerchantNotFound);
    }

    let merchant_id = get_merchant_id(env, merchant);
    let mut merchant_data: Merchant = env
        .storage()
        .persistent()
        .get(&DataKey::Merchant(merchant_id))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::MerchantNotFound));

    merchant_data.webhook = webhook.clone();

    env.storage()
        .persistent()
        .set(&DataKey::Merchant(merchant_id), &merchant_data);

    events::publish_merchant_webhook_set_event(
        env,
        merchant.clone(),
        merchant_id,
        webhook.clone(),
        env.ledger().timestamp(),
    );
}

pub fn get_merchant_webhook(env: &Env, merchant_id: u64) -> String {
    let merchant_data = get_merchant(env, merchant_id);
    merchant_data.webhook
}

pub fn is_token_accepted_for_merchant(env: &Env, merchant: &Address, token: &Address) -> bool {
    let merchant_tokens = get_merchant_accepted_tokens(env, merchant);

    // If merchant hasn't set any tokens, they accept all globally accepted tokens
    if merchant_tokens.is_empty() {
        return admin_component::is_accepted_token(env, token);
    }

    // Otherwise, check if it's in their specific list (which are already verified to be globally accepted)
    for merchant_token in merchant_tokens.iter() {
        if merchant_token == *token {
            return true;
        }
    }

    false
}
