use crate::components::{access_control, admin, merchant, signature_util};
use crate::errors::ContractError;
use crate::events;
use crate::types::{DataKey, Invoice, InvoiceFilter, InvoiceStatus, Role};
use soroban_sdk::token::TokenClient;
use soroban_sdk::{contractclient, panic_with_error, token, Address, BytesN, Env, String, Vec};

#[contractclient(name = "MerchantAccountRefundClient")]
pub trait MerchantAccountRefund {
    fn refund(env: Env, token: Address, amount: i128, to: Address);
}

pub const MAX_REFUND_DURATION: u64 = 604_800; // 7 days

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
    if !admin::is_accepted_token(env, token) {
        panic_with_error!(env, ContractError::TokenNotAccepted);
    }
    // check if expires_at is valid
    if let Some(expires_at) = expires_at {
        if expires_at < env.ledger().timestamp() {
            panic_with_error!(env, ContractError::InvoiceExpired);
        }
    }
    // check if amount is less than fee amount for the token
    let fee_amount = admin::get_fee(env, token);
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

pub fn refund_invoice_partial(env: &Env, invoice_id: u64, amount: i128) {
    let mut invoice = get_invoice(env, invoice_id);

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
            payer,
            invoice.amount,
            env.ledger().timestamp(),
        );
    } else {
        events::publish_invoice_partially_refunded_event(
            env,
            invoice_id,
            payer,
            amount,
            total_refund,
            env.ledger().timestamp(),
        );
    }
}

pub fn pay_invoice(env: &Env, payer: &Address, invoice_id: u64) -> i128 {
    let invoice = get_invoice(env, invoice_id);
    if invoice.status != InvoiceStatus::Pending && invoice.status != InvoiceStatus::PartiallyPaid {
        panic_with_error!(env, ContractError::InvalidInvoiceStatus);
    }
    let remaining_amount = invoice.amount - invoice.amount_paid;
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

    let fee_amount = admin::get_fee_for_amount(env, &invoice.token, amount);
    let merchant_amount = amount - fee_amount;

    let token_client = token::TokenClient::new(env, &invoice.token);
    let merchant_account_id = merchant::get_merchant_account(env, invoice.merchant_id);

    token_client.transfer(payer, &merchant_account_id, &merchant_amount);
    if fee_amount > 0 {
        token_client.transfer(payer, env.current_contract_address(), &fee_amount);
    }

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
