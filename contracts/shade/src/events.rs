use soroban_sdk::{contractevent, Address, BytesN, Env, String, Vec};

// ── Existing events ───────────────────────────────────────────────────────────

#[contractevent]
pub struct InitalizedEvent {
    pub admin: Address,
    pub timestamp: u64,
}

pub fn publish_initialized_event(env: &Env, admin: Address, timestamp: u64) {
    InitalizedEvent { admin, timestamp }.publish(env);
}
// no new changes to add

#[contractevent]
pub struct TokenAddedEvent {
    pub token: Address,
    pub timestamp: u64,
}

pub fn publish_token_added_event(env: &Env, token: Address, timestamp: u64) {
    TokenAddedEvent { token, timestamp }.publish(env);
}

#[contractevent]
pub struct TokenRemovedEvent {
    pub token: Address,
    pub timestamp: u64,
}

pub fn publish_token_removed_event(env: &Env, token: Address, timestamp: u64) {
    TokenRemovedEvent { token, timestamp }.publish(env);
}

#[contractevent]
pub struct MerchantRegisteredEvent {
    pub merchant: Address,
    pub merchant_id: u64,
    pub timestamp: u64,
}

pub fn publish_merchant_registered_event(
    env: &Env,
    merchant: Address,
    merchant_id: u64,
    timestamp: u64,
) {
    MerchantRegisteredEvent {
        merchant,
        merchant_id,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct MerchantAccountDeployedEvent {
    pub merchant: Address,
    pub contract: Address,
    pub timestamp: u64,
}

pub fn publish_merchant_account_deployed_event(
    env: &Env,
    merchant: Address,
    contract: Address,
    timestamp: u64,
) {
    MerchantAccountDeployedEvent {
        merchant,
        contract,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct MerchantStatusChangedEvent {
    pub merchant_id: u64,
    pub active: bool,
    pub timestamp: u64,
}

pub fn publish_merchant_status_changed_event(
    env: &Env,
    merchant_id: u64,
    active: bool,
    timestamp: u64,
) {
    MerchantStatusChangedEvent {
        merchant_id,
        active,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct InvoiceCreatedEvent {
    pub invoice_id: u64,
    pub merchant: Address,
    pub amount: i128,
    pub token: Address,
}

pub fn publish_invoice_created_event(
    env: &Env,
    invoice_id: u64,
    merchant: Address,
    amount: i128,
    token: Address,
) {
    InvoiceCreatedEvent {
        invoice_id,
        merchant,
        amount,
        token,
    }
    .publish(env);
}

#[contractevent]
pub struct InvoiceRefundedEvent {
    pub invoice_id: u64,
    pub merchant: Address,
    pub amount: i128,
    pub timestamp: u64,
}

pub fn publish_invoice_refunded_event(
    env: &Env,
    invoice_id: u64,
    merchant: Address,
    amount: i128,
    timestamp: u64,
) {
    InvoiceRefundedEvent {
        invoice_id,
        merchant,
        amount,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct InvoicePartiallyRefundedEvent {
    pub invoice_id: u64,
    pub merchant: Address,
    pub amount: i128,
    pub total_amount_refunded: i128,
    pub timestamp: u64,
}

pub fn publish_invoice_partially_refunded_event(
    env: &Env,
    invoice_id: u64,
    merchant: Address,
    amount: i128,
    total_amount_refunded: i128,
    timestamp: u64,
) {
    InvoicePartiallyRefundedEvent {
        invoice_id,
        merchant,
        amount,
        total_amount_refunded,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct MerchantVerifiedEvent {
    pub merchant_id: u64,
    pub status: bool,
    pub timestamp: u64,
}

pub fn publish_merchant_verified_event(env: &Env, merchant_id: u64, status: bool, timestamp: u64) {
    MerchantVerifiedEvent {
        merchant_id,
        status,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct MerchantWebhookSetEvent {
    pub merchant: Address,
    pub merchant_id: u64,
    pub webhook: String,
    pub timestamp: u64,
}

pub fn publish_merchant_webhook_set_event(
    env: &Env,
    merchant: Address,
    merchant_id: u64,
    webhook: String,
    timestamp: u64,
) {
    MerchantWebhookSetEvent {
        merchant,
        merchant_id,
        webhook,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct MerchantKeySetEvent {
    pub merchant: Address,
    pub key: BytesN<32>,
    pub timestamp: u64,
}

pub fn publish_merchant_key_set_event(
    env: &Env,
    merchant: Address,
    key: BytesN<32>,
    timestamp: u64,
) {
    MerchantKeySetEvent {
        merchant,
        key,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct RoleGrantedEvent {
    pub admin: Address,
    pub user: Address,
    pub role: crate::types::Role,
    pub timestamp: u64,
}

pub fn publish_role_granted_event(
    env: &Env,
    admin: Address,
    user: Address,
    role: crate::types::Role,
    timestamp: u64,
) {
    RoleGrantedEvent {
        admin,
        user,
        role,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct RoleRevokedEvent {
    pub admin: Address,
    pub user: Address,
    pub role: crate::types::Role,
    pub timestamp: u64,
}

pub fn publish_role_revoked_event(
    env: &Env,
    admin: Address,
    user: Address,
    role: crate::types::Role,
    timestamp: u64,
) {
    RoleRevokedEvent {
        admin,
        user,
        role,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct ContractPausedEvent {
    pub admin: Address,
    pub timestamp: u64,
}

pub fn publish_contract_paused_event(env: &Env, admin: Address, timestamp: u64) {
    ContractPausedEvent { admin, timestamp }.publish(env);
}

#[contractevent]
pub struct ContractUnpausedEvent {
    pub admin: Address,
    pub timestamp: u64,
}

pub fn publish_contract_unpaused_event(env: &Env, admin: Address, timestamp: u64) {
    ContractUnpausedEvent { admin, timestamp }.publish(env);
}

#[contractevent]
pub struct FeeProposedEvent {
    pub admin: Address,
    pub token: Address,
    pub fee: i128,
    pub timestamp: u64,
}

pub fn publish_fee_proposed_event(
    env: &Env,
    admin: Address,
    token: Address,
    fee: i128,
    timestamp: u64,
) {
    FeeProposedEvent {
        admin,
        token,
        fee,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct FeeSetEvent {
    pub admin: Address,
    pub token: Address,
    pub fee: i128,
    pub timestamp: u64,
}

pub fn publish_fee_set_event(env: &Env, admin: Address, token: Address, fee: i128, timestamp: u64) {
    FeeSetEvent {
        admin,
        token,
        fee,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct PlatformAccountSetEvent {
    pub admin: Address,
    pub account: Address,
    pub timestamp: u64,
}

pub fn publish_platform_account_set_event(
    env: &Env,
    admin: Address,
    account: Address,
    timestamp: u64,
) {
    PlatformAccountSetEvent {
        admin,
        account,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct TokenOracleSetEvent {
    pub admin: Address,
    pub token: Address,
    pub oracle: Address,
    pub timestamp: u64,
}

pub fn publish_token_oracle_set_event(
    env: &Env,
    admin: Address,
    token: Address,
    oracle: Address,
    timestamp: u64,
) {
    TokenOracleSetEvent {
        admin,
        token,
        oracle,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct ContractUpgradedEvent {
    pub new_wasm_hash: BytesN<32>,
    pub timestamp: u64,
}

pub fn publish_contract_upgraded_event(env: &Env, new_wasm_hash: BytesN<32>, timestamp: u64) {
    ContractUpgradedEvent {
        new_wasm_hash,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct AccountRestrictedEvent {
    pub merchant: Address,
    pub status: bool,
    pub caller: Address,
    pub timestamp: u64,
}

pub fn publish_account_restricted_event(
    env: &Env,
    merchant: Address,
    status: bool,
    caller: Address,
    timestamp: u64,
) {
    AccountRestrictedEvent {
        merchant,
        status,
        caller,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct FeeDiscountAppliedEvent {
    pub merchant: Address,
    pub volume: i128,
    pub discount_bps: i128,
    pub timestamp: u64,
}

pub fn publish_fee_discount_applied_event(
    env: &Env,
    merchant: Address,
    volume: i128,
    discount_bps: i128,
    timestamp: u64,
) {
    FeeDiscountAppliedEvent {
        merchant,
        volume,
        discount_bps,
        timestamp,
    }
    .publish(env);
}

// Kept merchant_amount from your branch AND merchant_account from main — both are useful.
#[contractevent]
pub struct InvoicePaidEvent {
    pub invoice_id: u64,
    pub merchant_id: u64,
    pub merchant_account: Address,
    pub payer: Address,
    pub amount: i128,
    pub fee: i128,
    pub merchant_amount: i128,
    pub token: Address,
    pub timestamp: u64,
}

#[allow(clippy::too_many_arguments)]
pub fn publish_invoice_paid_event(
    env: &Env,
    invoice_id: u64,
    merchant_id: u64,
    merchant_account: Address,
    payer: Address,
    amount: i128,
    fee: i128,
    merchant_amount: i128,
    token: Address,
    timestamp: u64,
) {
    InvoicePaidEvent {
        invoice_id,
        merchant_id,
        merchant_account,
        payer,
        amount,
        fee,
        merchant_amount,
        token,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct FiatInvoicePricedEvent {
    pub invoice_id: u64,
    pub token: Address,
    pub resolved_amount: i128,
    pub timestamp: u64,
}

pub fn publish_fiat_invoice_priced_event(
    env: &Env,
    invoice_id: u64,
    token: Address,
    resolved_amount: i128,
    timestamp: u64,
) {
    FiatInvoicePricedEvent {
        invoice_id,
        token,
        resolved_amount,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct PaymentSplitRoutedEvent {
    pub invoice_id: u64,
    pub merchant_account: Address,
    pub platform_account: Address,
    pub merchant_amount: i128,
    pub platform_amount: i128,
    pub token: Address,
    pub timestamp: u64,
}

pub fn publish_payment_split_routed_event(
    env: &Env,
    invoice_id: u64,
    merchant_account: Address,
    platform_account: Address,
    merchant_amount: i128,
    platform_amount: i128,
    token: Address,
    timestamp: u64,
) {
    PaymentSplitRoutedEvent {
        invoice_id,
        merchant_account,
        platform_account,
        merchant_amount,
        platform_amount,
        token,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct InvoiceCancelledEvent {
    pub invoice_id: u64,
    pub merchant: Address,
    pub timestamp: u64,
}

pub fn publish_invoice_cancelled_event(
    env: &Env,
    invoice_id: u64,
    merchant: Address,
    timestamp: u64,
) {
    InvoiceCancelledEvent {
        invoice_id,
        merchant,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct InvoiceAmendedEvent {
    pub invoice_id: u64,
    pub merchant: Address,
    pub old_amount: i128,
    pub new_amount: i128,
    pub timestamp: u64,
}

pub fn publish_invoice_amended_event(
    env: &Env,
    invoice_id: u64,
    merchant: Address,
    old_amount: i128,
    new_amount: i128,
    timestamp: u64,
) {
    InvoiceAmendedEvent {
        invoice_id,
        merchant,
        old_amount,
        new_amount,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct NonceInvalidatedEvent {
    pub merchant: Address,
    pub nonce: BytesN<32>,
    pub timestamp: u64,
}

pub fn publish_nonce_invalidated_event(
    env: &Env,
    merchant: Address,
    nonce: BytesN<32>,
    timestamp: u64,
) {
    NonceInvalidatedEvent {
        merchant,
        nonce,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct CrossChainBridgePlaceholderEvent {
    pub caller: Address,
    pub payload: crate::types::CrossChainBridgePayload,
    pub timestamp: u64,
}

pub fn publish_cross_chain_bridge_placeholder_event(
    env: &Env,
    caller: Address,
    payload: crate::types::CrossChainBridgePayload,
    timestamp: u64,
) {
    CrossChainBridgePlaceholderEvent {
        caller,
        payload,
        timestamp,
    }
    .publish(env);
}

// ── Subscription events ───────────────────────────────────────────────────────

// Kept token field from your branch (more informative than main's leaner version).
#[contractevent]
pub struct SubscriptionPlanCreatedEvent {
    pub plan_id: u64,
    pub merchant: Address,
    pub token: Address,
    pub amount: i128,
    pub interval: u64,
    pub timestamp: u64,
}

pub fn publish_subscription_plan_created_event(
    env: &Env,
    plan_id: u64,
    merchant: Address,
    token: Address,
    amount: i128,
    interval: u64,
    timestamp: u64,
) {
    SubscriptionPlanCreatedEvent {
        plan_id,
        merchant,
        token,
        amount,
        interval,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct SubscribedEvent {
    pub subscription_id: u64,
    pub plan_id: u64,
    pub customer: Address,
    pub timestamp: u64,
}

pub fn publish_subscribed_event(
    env: &Env,
    subscription_id: u64,
    plan_id: u64,
    customer: Address,
    timestamp: u64,
) {
    SubscribedEvent {
        subscription_id,
        plan_id,
        customer,
        timestamp,
    }
    .publish(env);
}

// Kept the richer version from your branch (plan_id, customer, merchant, token).
#[contractevent]
pub struct SubscriptionChargedEvent {
    pub subscription_id: u64,
    pub plan_id: u64,
    pub customer: Address,
    pub merchant: Address,
    pub amount: i128,
    pub fee: i128,
    pub token: Address,
    pub timestamp: u64,
}

#[allow(clippy::too_many_arguments)]
pub fn publish_subscription_charged_event(
    env: &Env,
    subscription_id: u64,
    plan_id: u64,
    customer: Address,
    merchant: Address,
    amount: i128,
    fee: i128,
    token: Address,
    timestamp: u64,
) {
    SubscriptionChargedEvent {
        subscription_id,
        plan_id,
        customer,
        merchant,
        amount,
        fee,
        token,
        timestamp,
    }
    .publish(env);
}

// Used "caller" from your branch — more accurate than "cancelled_by".
#[contractevent]
pub struct SubscriptionCancelledEvent {
    pub subscription_id: u64,
    pub caller: Address,
    pub timestamp: u64,
}

pub fn publish_subscription_cancelled_event(
    env: &Env,
    subscription_id: u64,
    caller: Address,
    timestamp: u64,
) {
    SubscriptionCancelledEvent {
        subscription_id,
        caller,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct AccountWasmHashSetEvent {
    pub admin: Address,
    pub wasm_hash: BytesN<32>,
    pub timestamp: u64,
}

pub fn publish_account_wasm_hash_set_event(
    env: &Env,
    admin: Address,
    wasm_hash: BytesN<32>,
    timestamp: u64,
) {
    AccountWasmHashSetEvent {
        admin,
        wasm_hash,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct PlanDeactivatedEvent {
    pub plan_id: u64,
    pub merchant: Address,
    pub timestamp: u64,
}

pub fn publish_plan_deactivated_event(env: &Env, plan_id: u64, merchant: Address, timestamp: u64) {
    PlanDeactivatedEvent {
        plan_id,
        merchant,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct MerchantTokensSetEvent {
    pub merchant: Address,
    pub tokens: Vec<Address>,
    pub timestamp: u64,
}

pub fn publish_merchant_tokens_set_event(
    env: &Env,
    merchant: Address,
    tokens: Vec<Address>,
    timestamp: u64,
) {
    MerchantTokensSetEvent {
        merchant,
        tokens,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct MerchantTokenRemovedEvent {
    pub merchant: Address,
    pub token: Address,
    pub timestamp: u64,
}

pub fn publish_merchant_token_removed_event(
    env: &Env,
    merchant: Address,
    token: Address,
    timestamp: u64,
) {
    MerchantTokenRemovedEvent {
        merchant,
        token,
        timestamp,
    }
    .publish(env);
}

// ── Admin transfer events ────────────────────────────────────────────────────

#[contractevent]
pub struct AdminTransferProposedEvent {
    pub current_admin: Address,
    pub proposed_admin: Address,
    pub timestamp: u64,
}

pub fn publish_admin_transfer_proposed_event(
    env: &Env,
    current_admin: Address,
    proposed_admin: Address,
    timestamp: u64,
) {
    AdminTransferProposedEvent {
        current_admin,
        proposed_admin,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct AdminTransferAcceptedEvent {
    pub old_admin: Address,
    pub new_admin: Address,
    pub timestamp: u64,
}

pub fn publish_admin_transfer_accepted_event(
    env: &Env,
    old_admin: Address,
    new_admin: Address,
    timestamp: u64,
) {
    AdminTransferAcceptedEvent {
        old_admin,
        new_admin,
        timestamp,
    }
    .publish(env);
}
