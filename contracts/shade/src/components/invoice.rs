use crate::components::{access_control, admin, history, merchant, signature_util};
use crate::errors::ContractError;
use crate::events;
use crate::types::{
    DataKey, FiatPricing, FiatPricingData, Invoice, InvoiceFilter, InvoicePricingMode,
    InvoiceStatus, Role, Transaction, TransactionType,
};
use soroban_sdk::token::TokenClient;
use soroban_sdk::{contractclient, panic_with_error, token, Address, BytesN, Env, String, Vec};

#[contractclient(name = "MerchantAccountRefundClient")]
pub trait MerchantAccountRefund {
    fn refund(env: Env, token: Address, amount: i128, to: Address);
}

#[contractclient(name = "PriceOracleClient")]
pub trait PriceOracle {
    fn get_price(env: Env, token: Address, quote_currency: String) -> i128;
}

pub const MAX_REFUND_DURATION: u64 = 604_800; // 7 days

fn scale_factor(decimals: u32) -> i128 {
    let mut factor = 1i128;
    for _ in 0..decimals {
        factor *= 10;
    }
    factor
}

fn resolve_fiat_invoice_amount(env: &Env, invoice: &Invoice) -> i128 {
    let fiat_pricing = match invoice.fiat_pricing.clone() {
        FiatPricingData::Some(fp) => fp,
        FiatPricingData::None => panic_with_error!(env, ContractError::OraclePriceUnavailable),
    };
    let oracle_config = admin::get_token_oracle(env, &invoice.token);
    let oracle_client = PriceOracleClient::new(env, &oracle_config.contract);
    let price = oracle_client.get_price(&invoice.token, &fiat_pricing.currency);

    if price <= 0 {
        panic_with_error!(env, ContractError::OraclePriceUnavailable);
    }

    let numerator = fiat_pricing.amount
        * scale_factor(oracle_config.token_decimals)
        * scale_factor(oracle_config.price_decimals);
    let denominator = price * scale_factor(fiat_pricing.decimals);
    let resolved_amount = numerator / denominator;

    if resolved_amount <= 0 {
        panic_with_error!(env, ContractError::OraclePriceUnavailable);
    }

    resolved_amount
}

fn refresh_fiat_invoice_quote(env: &Env, invoice: &mut Invoice) {
    if invoice.pricing_mode != InvoicePricingMode::FixedFiat || invoice.amount_paid > 0 {
        return;
    }

    let resolved_amount = resolve_fiat_invoice_amount(env, invoice);
    invoice.amount = resolved_amount;
    env.storage()
        .persistent()
        .set(&DataKey::Invoice(invoice.id), &*invoice);

    events::publish_fiat_invoice_priced_event(
        env,
        invoice.id,
        invoice.token.clone(),
        resolved_amount,
        env.ledger().timestamp(),
    );
}

pub fn validate_invoice_creation(
    env: &Env,
    merchant_address: &Address,
    description: &String,
    amount: i128,
    token: &Address,
    expires_at: Option<u64>,
) {
    if amount <= 0 {
        panic_with_error!(env, ContractError::InvalidAmount);
    }
    if description.len() > 100 {
        panic_with_error!(env, ContractError::InvalidDescription);
    }
    if !merchant::is_merchant(env, merchant_address) {
        panic_with_error!(env, ContractError::NotAuthorized);
    }
    // First, check global whitelist
    if !admin::is_accepted_token(env, token) {
        panic_with_error!(env, ContractError::TokenNotAccepted);
    }

    // Second, check merchant specific whitelist
    if !merchant::is_token_accepted_for_merchant(env, merchant_address, token) {
        panic_with_error!(env, ContractError::TokenNotAcceptedByMerchant);
    }
    // check if expires_at is valid
    if let Some(expires_at) = expires_at {
        if expires_at < env.ledger().timestamp() {
            panic_with_error!(env, ContractError::InvoiceExpired);
        }
    }
    // check if amount is less than fee amount for the token
    let merchant_address_val: Address = merchant_address.clone();
    let fee_amount = admin::calculate_fee(env, &merchant_address_val, token, amount);
    if amount <= fee_amount {
        panic_with_error!(env, ContractError::InvalidAmount);
    }

    let merchant_id: u64 = merchant::get_merchant_id(env, merchant_address);

    // ensure merchant is active
    if !merchant::is_merchant_active(env, merchant_id) {
        panic_with_error!(env, ContractError::MerchantNotActive);
    }
}

pub fn create_invoice(
    env: &Env,
    merchant_address: &Address,
    description: &String,
    amount: i128,
    token: &Address,
    expires_at: Option<u64>,
) -> u64 {
    merchant_address.require_auth();
    validate_invoice_creation(
        env,
        merchant_address,
        description,
        amount,
        token,
        expires_at,
    );

    let merchant_id: u64 = merchant::get_merchant_id(env, merchant_address);

    let invoice_count: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::InvoiceCount)
        .unwrap_or(0);
    let new_invoice_id = invoice_count + 1;
    let invoice = Invoice {
        id: new_invoice_id,
        description: description.clone(),
        amount,
        token: token.clone(),
        status: InvoiceStatus::Pending,
        merchant_id,
        payer: None,
        date_created: env.ledger().timestamp(),
        date_paid: None,
        amount_paid: 0,
        amount_refunded: 0,
        expires_at,
        pricing_mode: InvoicePricingMode::FixedCrypto,
        fiat_pricing: FiatPricingData::None,
    };
    env.storage()
        .persistent()
        .set(&DataKey::Invoice(new_invoice_id), &invoice);
    env.storage()
        .persistent()
        .set(&DataKey::InvoiceCount, &new_invoice_id);
    events::publish_invoice_created_event(
        env,
        new_invoice_id,
        merchant_address.clone(),
        amount,
        token.clone(),
    );
    new_invoice_id
}

pub fn create_fiat_invoice(
    env: &Env,
    merchant_address: &Address,
    description: &String,
    fiat_amount: i128,
    fiat_currency: &String,
    fiat_decimals: u32,
    token: &Address,
    expires_at: Option<u64>,
) -> u64 {
    merchant_address.require_auth();

    if fiat_amount <= 0 {
        panic_with_error!(env, ContractError::InvalidAmount);
    }

    let mut invoice = Invoice {
        id: 0,
        description: description.clone(),
        amount: 0,
        token: token.clone(),
        status: InvoiceStatus::Pending,
        merchant_id: merchant::get_merchant_id(env, merchant_address),
        payer: None,
        date_created: env.ledger().timestamp(),
        date_paid: None,
        amount_paid: 0,
        amount_refunded: 0,
        expires_at,
        pricing_mode: InvoicePricingMode::FixedFiat,
        fiat_pricing: FiatPricingData::Some(FiatPricing {
            currency: fiat_currency.clone(),
            amount: fiat_amount,
            decimals: fiat_decimals,
        }),
    };

    invoice.amount = resolve_fiat_invoice_amount(env, &invoice);

    validate_invoice_creation(
        env,
        merchant_address,
        description,
        invoice.amount,
        token,
        expires_at,
    );

    let invoice_count: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::InvoiceCount)
        .unwrap_or(0);
    let new_invoice_id = invoice_count + 1;
    invoice.id = new_invoice_id;

    env.storage()
        .persistent()
        .set(&DataKey::Invoice(new_invoice_id), &invoice);
    env.storage()
        .persistent()
        .set(&DataKey::InvoiceCount, &new_invoice_id);

    events::publish_invoice_created_event(
        env,
        new_invoice_id,
        merchant_address.clone(),
        invoice.amount,
        token.clone(),
    );
    events::publish_fiat_invoice_priced_event(
        env,
        new_invoice_id,
        token.clone(),
        invoice.amount,
        env.ledger().timestamp(),
    );

    new_invoice_id
}

pub fn create_invoice_draft(
    env: &Env,
    merchant_address: &Address,
    description: &String,
    amount: i128,
    token: &Address,
    expires_at: Option<u64>,
) -> u64 {
    merchant_address.require_auth();
    validate_invoice_creation(
        env,
        merchant_address,
        description,
        amount,
        token,
        expires_at,
    );

    let merchant_id: u64 = merchant::get_merchant_id(env, merchant_address);

    let invoice_count: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::InvoiceCount)
        .unwrap_or(0);
    let new_invoice_id = invoice_count + 1;
    let invoice = Invoice {
        id: new_invoice_id,
        description: description.clone(),
        amount,
        token: token.clone(),
        status: InvoiceStatus::Draft,
        merchant_id,
        payer: None,
        date_created: env.ledger().timestamp(),
        date_paid: None,
        amount_paid: 0,
        amount_refunded: 0,
        expires_at,
        pricing_mode: InvoicePricingMode::FixedCrypto,
        fiat_pricing: FiatPricingData::None,
    };
    env.storage()
        .persistent()
        .set(&DataKey::Invoice(new_invoice_id), &invoice);
    env.storage()
        .persistent()
        .set(&DataKey::InvoiceCount, &new_invoice_id);

    // We intentionally don't emit InvoiceCreatedEvent here since it's a draft

    new_invoice_id
}

pub fn finalize_invoice(env: &Env, merchant_address: &Address, invoice_id: u64) {
    merchant_address.require_auth();

    let mut invoice = get_invoice(env, invoice_id);

    let merchant_id: u64 = merchant::get_merchant_id(env, merchant_address);

    if invoice.merchant_id != merchant_id {
        panic_with_error!(env, ContractError::NotAuthorized);
    }

    if invoice.status != InvoiceStatus::Draft {
        panic_with_error!(env, ContractError::InvalidInvoiceStatus);
    }

    invoice.status = InvoiceStatus::Pending;

    env.storage()
        .persistent()
        .set(&DataKey::Invoice(invoice_id), &invoice);

    events::publish_invoice_created_event(
        env,
        invoice_id,
        merchant_address.clone(),
        invoice.amount,
        invoice.token.clone(),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn create_invoice_signed(
    env: &Env,
    caller: &Address,
    merchant: &Address,
    description: &String,
    amount: i128,
    token: &Address,
    nonce: &BytesN<32>,
    signature: &BytesN<64>,
) -> u64 {
    // Caller must be Manager or Admin
    if !access_control::has_role(env, caller, Role::Manager) {
        panic_with_error!(env, ContractError::NotAuthorized);
    }
    caller.require_auth();

    // validate invoice creation
    validate_invoice_creation(env, merchant, description, amount, token, None);

    // Verify merchant's cryptographic signature
    signature_util::verify_invoice_signature(
        env,
        merchant,
        description,
        amount,
        token,
        nonce,
        signature,
    );

    // Standard invoice creation
    let merchant_id: u64 = merchant::get_merchant_id(env, merchant);

    let invoice_count: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::InvoiceCount)
        .unwrap_or(0);

    let new_invoice_id = invoice_count + 1;

    let invoice = Invoice {
        id: new_invoice_id,
        description: description.clone(),
        amount,
        token: token.clone(),
        status: InvoiceStatus::Pending,
        merchant_id,
        payer: None,
        date_created: env.ledger().timestamp(),
        date_paid: None,
        amount_paid: 0,
        amount_refunded: 0,
        expires_at: None,
        pricing_mode: InvoicePricingMode::FixedCrypto,
        fiat_pricing: FiatPricingData::None,
    };

    env.storage()
        .persistent()
        .set(&DataKey::Invoice(new_invoice_id), &invoice);
    env.storage()
        .persistent()
        .set(&DataKey::InvoiceCount, &new_invoice_id);

    // 7. Emit standardInvoiceCreated event
    events::publish_invoice_created_event(
        env,
        new_invoice_id,
        merchant.clone(),
        amount,
        token.clone(),
    );

    new_invoice_id
}

pub fn get_invoice(env: &Env, invoice_id: u64) -> Invoice {
    env.storage()
        .persistent()
        .get(&DataKey::Invoice(invoice_id))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::InvoiceNotFound))
}

pub fn resolve_invoice_amount(env: &Env, invoice_id: u64) -> i128 {
    let invoice = get_invoice(env, invoice_id);
    if invoice.pricing_mode == InvoicePricingMode::FixedFiat && invoice.amount_paid == 0 {
        return resolve_fiat_invoice_amount(env, &invoice);
    }

    invoice.amount
}

pub fn check_invoice_refund_eligibility(env: &Env, merchant_address: &Address, invoice_id: u64) {
    let invoice = get_invoice(env, invoice_id);

    let merchant_id = merchant::get_merchant_id(env, merchant_address);

    if invoice.merchant_id != merchant_id {
        panic_with_error!(env, ContractError::NotAuthorized);
    }

    // check if the payer is available
    if invoice.payer.is_none() {
        panic_with_error!(env, ContractError::PayerNotAvailable);
    }

    // check if invoice is paid
    if invoice.status != InvoiceStatus::Paid {
        panic_with_error!(env, ContractError::InvoiceNotPaid);
    }

    // Enforce refund window
    if let Some(date_paid) = invoice.date_paid {
        let elapsed = env.ledger().timestamp() - date_paid;
        if elapsed > MAX_REFUND_DURATION {
            panic_with_error!(env, ContractError::RefundPeriodExpired);
        }
    } else {
        panic_with_error!(env, ContractError::InvoiceNotPaid);
    }

    let amount_to_refund = invoice.amount - invoice.amount_refunded;
    if amount_to_refund <= 0 {
        panic_with_error!(env, ContractError::InvalidAmount);
    }
}

pub fn refund_invoice(env: &Env, merchant_address: &Address, invoice_id: u64) {
    merchant_address.require_auth();

    check_invoice_refund_eligibility(env, merchant_address, invoice_id);

    // initiate refund
    let invoice = get_invoice(env, invoice_id);
    let amount_to_refund = invoice.amount - invoice.amount_refunded;

    let payer = invoice.payer.unwrap();
    // transfer amount_to_refund from merchant account to payer
    // check if merchant account balance for the token is sufficient
    let merchant_account = merchant::get_merchant_account(env, invoice.merchant_id);
    let token_client = TokenClient::new(env, &invoice.token);
    let merchant_balance = token_client.balance(&merchant_account);
    if merchant_balance < amount_to_refund {
        panic_with_error!(env, ContractError::InsufficientBalance);
    }
    let refund_client = MerchantAccountRefundClient::new(env, &merchant_account);
    refund_client.refund(&invoice.token, &amount_to_refund, &payer);

    // update invoice
    let mut invoice = get_invoice(env, invoice_id);
    invoice.amount_refunded += amount_to_refund;
    invoice.status = InvoiceStatus::Refunded;
    env.storage()
        .persistent()
        .set(&DataKey::Invoice(invoice_id), &invoice);

    events::publish_invoice_refunded_event(
        env,
        invoice_id,
        payer,
        invoice.amount,
        env.ledger().timestamp(),
    );
}

pub fn get_invoices(env: &Env, filter: InvoiceFilter) -> Vec<Invoice> {
    let invoice_count: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::InvoiceCount)
        .unwrap_or(0);
    let mut invoices: Vec<Invoice> = Vec::new(env);
    for i in 1..=invoice_count {
        if let Some(invoice) = env
            .storage()
            .persistent()
            .get::<_, Invoice>(&DataKey::Invoice(i))
        {
            let mut matches = true;
            if let Some(status) = filter.status {
                if invoice.status as u32 != status {
                    matches = false;
                }
            }
            if let Some(merchant) = &filter.merchant {
                if let Some(merchant_id) = env
                    .storage()
                    .persistent()
                    .get::<_, u64>(&DataKey::MerchantId(merchant.clone()))
                {
                    if invoice.merchant_id != merchant_id {
                        matches = false;
                    }
                } else {
                    matches = false;
                }
            }
            if let Some(min_amount) = filter.min_amount {
                if invoice.amount < min_amount as i128 {
                    matches = false;
                }
            }
            if let Some(max_amount) = filter.max_amount {
                if invoice.amount > max_amount as i128 {
                    matches = false;
                }
            }
            if let Some(start_date) = filter.start_date {
                if invoice.date_created < start_date {
                    matches = false;
                }
            }
            if let Some(end_date) = filter.end_date {
                if invoice.date_created > end_date {
                    matches = false;
                }
            }
            if matches {
                invoices.push_back(invoice);
            }
        }
    }
    invoices
}
//no new changes to add

pub fn refund_invoice_partial(env: &Env, merchant_address: &Address, invoice_id: u64, amount: i128) {
    merchant_address.require_auth();
    let mut invoice = get_invoice(env, invoice_id);

    let merchant_id = merchant::get_merchant_id(env, merchant_address);
    if invoice.merchant_id != merchant_id {
        panic_with_error!(env, ContractError::NotAuthorized);
    }

    if invoice.status != InvoiceStatus::Paid && invoice.status != InvoiceStatus::PartiallyRefunded {
        panic_with_error!(env, ContractError::InvalidInvoiceStatus);
    }

    if let Some(date_paid) = invoice.date_paid {
        let elapsed = env.ledger().timestamp() - date_paid;
        if elapsed > MAX_REFUND_DURATION {
            panic_with_error!(env, ContractError::RefundPeriodExpired);
        }
    }

    if amount <= 0 {
        panic_with_error!(env, ContractError::InvalidAmount);
    }

    let total_refund = invoice.amount_refunded + amount;
    if total_refund > invoice.amount {
        panic_with_error!(env, ContractError::InvalidAmount);
    }

    invoice.amount_refunded = total_refund;

    let new_status = if total_refund == invoice.amount {
        InvoiceStatus::Refunded
    } else {
        InvoiceStatus::PartiallyRefunded
    };
    invoice.status = new_status;

    // save invoice to storage
    env.storage()
        .persistent()
        .set(&DataKey::Invoice(invoice_id), &invoice);

    let payer = invoice
        .payer
        .clone()
        .unwrap_or_else(|| panic_with_error!(env, ContractError::PayerNotAvailable));

    let merchant_account_addr = merchant::get_merchant_account(env, invoice.merchant_id);
    // check if merchant account balance for the token is sufficient
    let token_client = TokenClient::new(env, &invoice.token);
    let merchant_balance = token_client.balance(&merchant_account_addr);
    if merchant_balance < amount {
        panic_with_error!(env, ContractError::InsufficientBalance);
    }
    // initiate refund
    let refund_client = MerchantAccountRefundClient::new(env, &merchant_account_addr);
    refund_client.refund(&invoice.token, &amount, &payer);

    if total_refund == invoice.amount {
        events::publish_invoice_refunded_event(
            env,
            invoice_id,
            merchant_address.clone(),
            invoice.amount,
            env.ledger().timestamp(),
        );
    } else {
        events::publish_invoice_partially_refunded_event(
            env,
            invoice_id,
            merchant_address.clone(),
            amount,
            total_refund,
            env.ledger().timestamp(),
        );
    }
}

pub fn pay_invoices_batch(env: &Env, payer: &Address, invoice_ids: &Vec<u64>) {
    payer.require_auth();
    for invoice_id in invoice_ids.iter() {
        pay_invoice(env, payer, invoice_id);
    }
}

pub fn pay_invoice(env: &Env, payer: &Address, invoice_id: u64) -> i128 {
    let invoice = get_invoice(env, invoice_id);
    if invoice.status != InvoiceStatus::Pending && invoice.status != InvoiceStatus::PartiallyPaid {
        panic_with_error!(env, ContractError::InvalidInvoiceStatus);
    }
    let remaining_amount = resolve_invoice_amount(env, invoice_id) - invoice.amount_paid;
    if remaining_amount <= 0 {
        panic_with_error!(env, ContractError::InvalidInvoiceStatus);
    }
    pay_invoice_partial(env, payer, invoice_id, remaining_amount)
}

pub fn pay_invoice_partial(env: &Env, payer: &Address, invoice_id: u64, amount: i128) -> i128 {
    payer.require_auth();

    if amount <= 0 {
        panic_with_error!(env, ContractError::InvalidAmount);
    }

    let mut invoice = get_invoice(env, invoice_id);
    refresh_fiat_invoice_quote(env, &mut invoice);

    if let Some(expires_at) = invoice.expires_at {
        if env.ledger().timestamp() >= expires_at {
            panic_with_error!(env, ContractError::InvoiceExpired);
        }
    }

    if invoice.status != InvoiceStatus::Pending && invoice.status != InvoiceStatus::PartiallyPaid {
        panic_with_error!(env, ContractError::InvalidInvoiceStatus);
    }

    if invoice.amount_paid + amount > invoice.amount {
        panic_with_error!(env, ContractError::InvalidAmount);
    }

    if !admin::is_accepted_token(env, &invoice.token) {
        panic_with_error!(env, ContractError::TokenNotAccepted);
    }

    let merchant_address: Address = merchant_id_to_address(env, invoice.merchant_id);
    let fee_amount = admin::calculate_fee(env, &merchant_address, &invoice.token, amount);
    let merchant_account_id = merchant::get_merchant_account(env, invoice.merchant_id);
    let platform_account = admin::get_platform_account(env);
    let merchant_amount = amount - fee_amount;

    let token_client = token::TokenClient::new(env, &invoice.token);

    token_client.transfer(payer, &merchant_account_id, &merchant_amount);
    if fee_amount > 0 {
        token_client.transfer(payer, &platform_account, &fee_amount);
    }
    admin::record_merchant_payment(env, &merchant_address, &invoice.token, amount, fee_amount);

    invoice.amount_paid += amount;
    if let Some(existing_payer) = &invoice.payer {
        if *existing_payer != *payer {
            panic_with_error!(env, ContractError::NotAuthorized);
        }
    } else {
        invoice.payer = Some(payer.clone());
    }

    if invoice.amount_paid == invoice.amount {
        invoice.status = InvoiceStatus::Paid;
        invoice.date_paid = Some(env.ledger().timestamp());
    } else {
        invoice.status = InvoiceStatus::PartiallyPaid;
    }

    env.storage()
        .persistent()
        .set(&DataKey::Invoice(invoice_id), &invoice);

    events::publish_invoice_paid_event(
        env,
        invoice_id,
        invoice.merchant_id,
        merchant_account_id.clone(),
        payer.clone(),
        amount,
        fee_amount,
        merchant_amount,
        invoice.token.clone(),
        env.ledger().timestamp(),
    );
    events::publish_payment_split_routed_event(
        env,
        invoice_id,
        merchant_account_id,
        platform_account,
        merchant_amount,
        fee_amount,
        invoice.token.clone(),
        env.ledger().timestamp(),
    );

    let transaction = Transaction {
        transaction_type: TransactionType::InvoicePayment,
        ref_id: invoice_id,
        amount,
        token: invoice.token.clone(),
        description: invoice.description.clone(),
        date: env.ledger().timestamp(),
        merchant_id: invoice.merchant_id,
    };
    history::record_transaction(env, payer, transaction);

    fee_amount
}

pub fn void_invoice(env: &Env, merchant_address: &Address, invoice_id: u64) {
    merchant_address.require_auth();

    let mut invoice = get_invoice(env, invoice_id);

    let merchant_id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::MerchantId(merchant_address.clone()))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::NotAuthorized));

    if invoice.merchant_id != merchant_id {
        panic_with_error!(env, ContractError::NotAuthorized);
    }

    if invoice.status != InvoiceStatus::Pending {
        panic_with_error!(env, ContractError::InvalidInvoiceStatus);
    }

    invoice.status = InvoiceStatus::Cancelled;

    env.storage()
        .persistent()
        .set(&DataKey::Invoice(invoice_id), &invoice);

    events::publish_invoice_cancelled_event(
        env,
        invoice_id,
        merchant_address.clone(),
        env.ledger().timestamp(),
    );
}

pub fn amend_invoice(
    env: &Env,
    merchant_address: &Address,
    invoice_id: u64,
    new_amount: Option<i128>,
    new_description: Option<String>,
) {
    merchant_address.require_auth();

    let mut invoice = get_invoice(env, invoice_id);

    let merchant_id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::MerchantId(merchant_address.clone()))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::NotAuthorized));

    if invoice.merchant_id != merchant_id {
        panic_with_error!(env, ContractError::NotAuthorized);
    }

    if invoice.status != InvoiceStatus::Pending {
        panic_with_error!(env, ContractError::InvalidInvoiceStatus);
    }

    let old_amount = invoice.amount;

    if let Some(amount) = new_amount {
        if amount <= 0 {
            panic_with_error!(env, ContractError::InvalidAmount);
        }
        invoice.amount = amount;
    }

    if let Some(description) = new_description {
        invoice.description = description;
    }

    env.storage()
        .persistent()
        .set(&DataKey::Invoice(invoice_id), &invoice);

    events::publish_invoice_amended_event(
        env,
        invoice_id,
        merchant_address.clone(),
        old_amount,
        invoice.amount,
        env.ledger().timestamp(),
    );
}

fn merchant_id_to_address(env: &Env, merchant_id: u64) -> Address {
    let merchant_data: crate::types::Merchant = env
        .storage()
        .persistent()
        .get(&crate::types::DataKey::Merchant(merchant_id))
        .unwrap_or_else(|| panic_with_error!(env, ContractError::MerchantNotFound));
    merchant_data.address
}
