use crate::types::{DataKey, Transaction};
use soroban_sdk::{Address, Env, Vec};

pub fn record_transaction(env: &Env, user: &Address, transaction: Transaction) {
    let mut history: Vec<Transaction> = env
        .storage()
        .persistent()
        .get(&DataKey::UserTransactions(user.clone()))
        .unwrap_or_else(|| Vec::new(env));

    history.push_back(transaction);
    env.storage()
        .persistent()
        .set(&DataKey::UserTransactions(user.clone()), &history);
}

pub fn get_user_transactions(env: &Env, user: Address) -> Vec<Transaction> {
    env.storage()
        .persistent()
        .get(&DataKey::UserTransactions(user))
        .unwrap_or_else(|| Vec::new(env))
}
