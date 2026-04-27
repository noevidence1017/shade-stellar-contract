use soroban_sdk::{contracttype, Address, Vec};

#[contracttype]
pub enum DataKey {
    Manager,
    Merchant,
    Verified,
    Restricted,
    AccountInfo,
    TrackedTokens,
    WithdrawalAnalytics(Address),
    Threshold,
    WithdrawalRequest(u64),
    WithdrawalCount,
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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawalRequest {
    pub id: u64,
    pub token: Address,
    pub amount: i128,
    pub recipient: Address,
    pub approvals: Vec<Address>,
    pub status: WithdrawalStatus,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum WithdrawalStatus {
    Pending = 0,
    Approved = 1,
    Executed = 2,
}
