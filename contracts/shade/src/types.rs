use soroban_sdk::{contracttype, Address, BytesN, String};

#[contracttype]
pub enum DataKey {
    Admin,
    PendingAdmin,
    Paused,
    FeeInBasisPoints(Address),
    FeeAmount(Address),
    ContractInfo,
    AcceptedTokens,
    Merchant(u64),
    MerchantKey(Address),
    MerchantCount,
    MerchantId(Address),
    TokenFee(Address),
    MerchantTokens(Address),
    MerchantBalance(Address),
    MerchantAccount(u64),
    Invoice(u64),
    InvoiceCount,
    ReentrancyStatus,
    AccountWasmHash,
    Role(Address, Role),
    UsedNonce(Address, BytesN<32>),
    // --- Subscription engine ---
    SubscriptionPlan(u64),
    Subscription(u64),
    PlanCount,
    SubscriptionCount,
    // --- Time-locked fee updates ---
    PendingTokenFee(Address),
    // --- Fee discount system ---
    MerchantVolume(Address, Address),
    UserTransactions(Address),
    MerchantAnalytics(Address, Address),
    MerchantAnalyticsSummary(Address),
    PlatformAccount,
    TokenOracle(Address),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractInfo {
    pub admin: Address,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Merchant {
    pub id: u64,
    pub address: Address,
    pub active: bool,
    pub verified: bool,
    pub date_registered: u64,
    pub account: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Invoice {
    pub id: u64,
    pub description: soroban_sdk::String,
    pub amount: i128,
    pub token: Address,
    pub status: InvoiceStatus,
    pub merchant_id: u64,
    pub payer: Option<Address>,
    pub date_created: u64,
    pub date_paid: Option<u64>,
    pub amount_paid: i128,
    pub amount_refunded: i128,
    pub expires_at: Option<u64>,
    pub pricing_mode: InvoicePricingMode,
    pub fiat_pricing: Option<FiatPricing>,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum InvoiceStatus {
    Pending = 0,
    Paid = 1,
    Cancelled = 2,
    Refunded = 3,
    PartiallyRefunded = 4,
    PartiallyPaid = 5,
    Draft = 6,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum InvoicePricingMode {
    FixedCrypto = 0,
    FixedFiat = 1,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FiatPricing {
    pub currency: String,
    pub amount: i128,
    pub decimals: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MerchantFilter {
    pub is_active: Option<bool>,
    pub is_verified: Option<bool>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvoiceFilter {
    pub status: Option<u32>,
    pub merchant: Option<Address>,
    pub min_amount: Option<u128>,
    pub max_amount: Option<u128>,
    pub start_date: Option<u64>,
    pub end_date: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Role {
    Admin,
    Manager,
    Operator,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolumeDiscount {
    pub min_volume: i128,
    pub discount_bps: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleConfig {
    pub contract: Address,
    pub price_decimals: u32,
    pub token_decimals: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MerchantAnalytics {
    pub merchant: Address,
    pub token: Address,
    pub total_volume: i128,
    pub total_fees: i128,
    pub transaction_count: u64,
    pub last_updated: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MerchantAnalyticsSummary {
    pub merchant: Address,
    pub total_volume: i128,
    pub total_fees: i128,
    pub transaction_count: u64,
    pub last_updated: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossChainBridgePayload {
    pub invoice_id: u64,
    pub merchant: Address,
    pub payer: Option<Address>,
    pub source_chain: String,
    pub destination_chain: String,
    pub token: Address,
    pub amount: i128,
    pub destination_recipient: String,
    pub memo: Option<String>,
}

// ── Time-locked fee update ────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingFee {
    pub token: Address,
    pub fee: i128,
    pub proposed_at: u64,
}
// ── Subscription engine ───────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionPlan {
    pub id: u64,
    /// Numeric merchant ID — used to look up the merchant's account contract.
    pub merchant_id: u64,
    /// The merchant's wallet address — needed for event emission and auth checks.
    pub merchant: Address,
    /// Human-readable description of the plan.
    pub description: soroban_sdk::String,
    /// Token used for billing.
    pub token: Address,
    /// Amount charged per interval (in token base units).
    pub amount: i128,
    /// Billing interval in seconds (e.g. 2_592_000 = 30 days).
    pub interval: u64,
    /// Whether this plan is accepting new subscribers.
    pub active: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Subscription {
    pub id: u64,
    pub plan_id: u64,
    pub customer: Address,
    /// Copied from the plan for quick access during auth checks.
    pub merchant_id: u64,
    pub status: SubscriptionStatus,
    pub date_created: u64,
    /// Ledger timestamp of the last successful charge.
    /// Starts at 0 so the first charge is available immediately.
    pub last_charged: u64,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SubscriptionStatus {
    Active = 0,
    Cancelled = 1,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransactionType {
    InvoicePayment = 0,
    SubscriptionCharge = 1,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transaction {
    pub transaction_type: TransactionType,
    pub ref_id: u64,
    pub amount: i128,
    pub token: Address,
    pub description: soroban_sdk::String,
    pub date: u64,
    pub merchant_id: u64,
}
