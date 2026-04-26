use crate::errors::ContractError;
use crate::events::{
    publish_account_initialized_event, publish_account_restricted_event,
    publish_account_verified_event, publish_refund_processed_event, publish_token_added_event,
    publish_withdrawal_to_event,
};
use crate::interface::MerchantAccountTrait;
use crate::types::{AccountInfo, DataKey, TokenBalance};
use soroban_sdk::{contract, contractimpl, panic_with_error, token, Address, Env, Vec};

#[contract]
pub struct MerchantAccount;

fn get_manager(env: &Env) -> Address {
    env.storage()
        .persistent()
        .get(&DataKey::Manager)
        .unwrap_or_else(|| panic_with_error!(env, ContractError::NotInitialized))
}

fn get_tracked_tokens(env: &Env) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::TrackedTokens)
        .unwrap_or_else(|| Vec::new(env))
}

fn is_restricted_account(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::Restricted)
        .unwrap_or(false)
}

fn token_exists(tracked_tokens: &Vec<Address>, token: &Address) -> bool {
    for tracked_token in tracked_tokens.iter() {
        if tracked_token == token.clone() {
            return true;
        }
    }
    false
}

#[contractimpl]
impl MerchantAccountTrait for MerchantAccount {
    fn initialize(env: Env, merchant: Address, manager: Address, merchant_id: u64) {
        if env.storage().persistent().has(&DataKey::Merchant) {
            panic_with_error!(&env, ContractError::AlreadyInitialized);
        }
        let account_info = AccountInfo {
            merchant: merchant.clone(),
            manager: manager.clone(),
            merchant_id,
            date_created: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::AccountInfo, &account_info);
        env.storage()
            .persistent()
            .set(&DataKey::Merchant, &merchant);
        env.storage().persistent().set(&DataKey::Manager, &manager);
        publish_account_initialized_event(
            &env,
            merchant.clone(),
            merchant_id,
            env.ledger().timestamp(),
        );
    }
    fn get_merchant(env: Env) -> Address {
        env.storage()
            .persistent()
            .get(&DataKey::Merchant)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::NotInitialized))
    }

    fn add_token(env: Env, token: Address) {
        let manager = get_manager(&env);
        manager.require_auth();

        let mut tracked_tokens = get_tracked_tokens(&env);
        if token_exists(&tracked_tokens, &token) {
            return;
        }

        tracked_tokens.push_back(token.clone());
        env.storage()
            .persistent()
            .set(&DataKey::TrackedTokens, &tracked_tokens);
        publish_token_added_event(&env, token, env.ledger().timestamp());
    }

    fn refund(env: Env, token: Address, amount: i128, to: Address) {
        let manager = get_manager(&env);
        manager.require_auth();

        if is_restricted_account(&env) {
            panic_with_error!(&env, ContractError::AccountRestricted);
        }

        let contract_address = env.current_contract_address();
        let token_client = token::TokenClient::new(&env, &token);
        token_client.transfer(&contract_address, &to, &amount);

        publish_refund_processed_event(&env, token, amount, to, env.ledger().timestamp());
    }

    fn has_token(env: Env, token: Address) -> bool {
        let tracked_tokens = get_tracked_tokens(&env);
        token_exists(&tracked_tokens, &token)
    }

    fn get_balance(env: Env, token: Address) -> i128 {
        let token_client = token::TokenClient::new(&env, &token);
        token_client.balance(&env.current_contract_address())
    }

    fn get_balances(env: Env) -> Vec<TokenBalance> {
        let tracked_tokens = get_tracked_tokens(&env);
        let contract_address = env.current_contract_address();
        let mut balances = Vec::new(&env);

        for tracked_token in tracked_tokens.iter() {
            let balance = token::TokenClient::new(&env, &tracked_token).balance(&contract_address);
            balances.push_back(TokenBalance {
                token: tracked_token,
                balance,
            });
        }

        balances
    }

    fn verify_account(env: Env) {
        let manager = get_manager(&env);
        manager.require_auth();

        env.storage().persistent().set(&DataKey::Verified, &true);
        publish_account_verified_event(&env, env.ledger().timestamp());
    }

    fn is_verified_account(env: Env) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Verified)
            .unwrap_or(false)
    }

    fn restrict_account(env: Env, status: bool) {
        let manager = get_manager(&env);
        manager.require_auth();

        env.storage()
            .persistent()
            .set(&DataKey::Restricted, &status);
        publish_account_restricted_event(&env, status, env.ledger().timestamp());
    }

    fn is_restricted_account(env: Env) -> bool {
        is_restricted_account(&env)
    }

    fn withdraw_to(env: Env, token: Address, amount: i128, recipient: Address) {
        // Only the merchant can initiate withdrawals to another account
        let merchant = Self::get_merchant(env.clone());
        merchant.require_auth();

        let token_client = token::TokenClient::new(&env, &token);
        let current_balance = token_client.balance(&env.current_contract_address());

        if amount > current_balance {
            panic_with_error!(&env, ContractError::InsufficientBalance);
        }

        if is_restricted_account(&env) {
            panic_with_error!(&env, ContractError::AccountRestricted);
        }

        token_client.transfer(&env.current_contract_address(), &recipient, &amount);

        publish_withdrawal_to_event(
            &env,
            token,
            merchant,
            recipient,
            amount,
            env.ledger().timestamp(),
        );
    }
}
