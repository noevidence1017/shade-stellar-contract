use crate::types::{TokenBalance, WithdrawalAnalytics};
use soroban_sdk::{contracttrait, Address, Env, Vec};

#[contracttrait]
pub trait MerchantAccountTrait {
    fn initialize(env: Env, merchant: Address, manager: Address, merchant_id: u64);
    fn get_merchant(env: Env) -> Address;
    fn add_token(env: Env, token: Address);
    fn refund(env: Env, token: Address, amount: i128, to: Address);
    fn has_token(env: Env, token: Address) -> bool;
    fn get_balance(env: Env, token: Address) -> i128;
    fn get_balances(env: Env) -> Vec<TokenBalance>;
    fn get_withdrawal_analytics(env: Env, token: Address) -> WithdrawalAnalytics;
    fn verify_account(env: Env);
    fn is_verified_account(env: Env) -> bool;
    fn restrict_account(env: Env, status: bool);
    fn is_restricted_account(env: Env) -> bool;
    fn withdraw_to(env: Env, token: Address, amount: i128, recipient: Address);
}
