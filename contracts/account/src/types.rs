use soroban_sdk::{contracttype, Address};

#[contracttype]
pub enum DataKey {
    Manager,
    Merchant,
    Verified,
    Restricted,
    AccountInfo,
    TrackedTokens,
    WithdrawalAnalytics(Address),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountInfo {
    pub manager: Address,
    pub merchant_id: u64,
    pub merchant: Address,
    pub date_created: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenBalance {
    pub token: Address,
    pub balance: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawalAnalytics {
    pub token: Address,
    pub total_withdrawn: i128,
    pub withdrawal_count: u64,
    pub last_withdrawn_at: u64,
}
