#![cfg(test)]

//! Comprehensive tests for date range filtering on invoices.
//!
//! These tests exercise the `start_date` and `end_date` fields of
//! `InvoiceFilter` in isolation and in combination with other filters.

use crate::shade::{Shade, ShadeClient};
use crate::types::{InvoiceFilter, InvoiceStatus};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{Address, Env, String};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup_env() -> (Env, ShadeClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, contract_id, admin)
}

fn make_token(env: &Env, admin: &Address) -> Address {
    env.register_stellar_asset_contract_v2(admin.clone()).address()
}

/// Register a merchant, add a token, and create invoices at the given
/// timestamps. Returns (merchant, token, invoice_ids).
fn create_invoices_at(
    env: &Env,
    client: &ShadeClient<'_>,
    admin: &Address,
    timestamps_and_amounts: &[(u64, i128)],
) -> (Address, Address, soroban_sdk::Vec<u64>) {
    let merchant = Address::generate(env);
    client.register_merchant(&merchant);

    let token = make_token(env, admin);
    client.add_accepted_token(admin, &token);

    let mut ids = soroban_sdk::Vec::new(env);
    for (ts, amount) in timestamps_and_amounts {
        env.ledger().set_timestamp(*ts);
        let id = client.create_invoice(
            &merchant,
            &String::from_str(env, "Test invoice"),
            amount,
            &token,
            &None,
        );
        ids.push_back(id);
    }
    (merchant, token, ids)
}

fn no_filter() -> InvoiceFilter {
    InvoiceFilter {
        status: None,
        merchant: None,
        min_amount: None,
        max_amount: None,
        start_date: None,
        end_date: None,
    }
}

// ---------------------------------------------------------------------------
// Basic date range tests
// ---------------------------------------------------------------------------

#[test]
fn test_no_date_filter_returns_all_invoices() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(&env, &client, &admin, &[(1000, 100), (2000, 200), (3000, 300)]);

    let result = client.get_invoices(&no_filter());
    assert_eq!(result.len(), 3);
}

#[test]
fn test_start_date_excludes_earlier_invoices() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(&env, &client, &admin, &[(1000, 100), (2000, 200), (3000, 300)]);

    let filter = InvoiceFilter { start_date: Some(2000), ..no_filter() };
    let result = client.get_invoices(&filter);

    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|inv| inv.date_created >= 2000));
}

#[test]
fn test_end_date_excludes_later_invoices() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(&env, &client, &admin, &[(1000, 100), (2000, 200), (3000, 300)]);

    let filter = InvoiceFilter { end_date: Some(2000), ..no_filter() };
    let result = client.get_invoices(&filter);

    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|inv| inv.date_created <= 2000));
}

#[test]
fn test_date_range_returns_only_invoices_within_window() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(&env, &client, &admin, &[(1000, 100), (2000, 200), (3000, 300)]);

    let filter = InvoiceFilter {
        start_date: Some(1500),
        end_date: Some(2500),
        ..no_filter()
    };
    let result = client.get_invoices(&filter);

    assert_eq!(result.len(), 1);
    assert_eq!(result.get(0).unwrap().date_created, 2000);
}

#[test]
fn test_start_date_equals_invoice_timestamp_is_inclusive() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(&env, &client, &admin, &[(1000, 100), (2000, 200)]);

    let filter = InvoiceFilter { start_date: Some(1000), ..no_filter() };
    let result = client.get_invoices(&filter);

    assert_eq!(result.len(), 2);
    assert_eq!(result.get(0).unwrap().date_created, 1000);
}

#[test]
fn test_end_date_equals_invoice_timestamp_is_inclusive() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(&env, &client, &admin, &[(1000, 100), (2000, 200)]);

    let filter = InvoiceFilter { end_date: Some(2000), ..no_filter() };
    let result = client.get_invoices(&filter);

    assert_eq!(result.len(), 2);
    assert_eq!(result.get(1).unwrap().date_created, 2000);
}

#[test]
fn test_single_point_range_matches_exact_timestamp() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(&env, &client, &admin, &[(1000, 100), (2000, 200), (3000, 300)]);

    let filter = InvoiceFilter {
        start_date: Some(2000),
        end_date: Some(2000),
        ..no_filter()
    };
    let result = client.get_invoices(&filter);

    assert_eq!(result.len(), 1);
    assert_eq!(result.get(0).unwrap().date_created, 2000);
}

#[test]
fn test_range_before_all_invoices_returns_empty() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(&env, &client, &admin, &[(1000, 100), (2000, 200)]);

    let filter = InvoiceFilter {
        start_date: Some(1),
        end_date: Some(999),
        ..no_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_range_after_all_invoices_returns_empty() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(&env, &client, &admin, &[(1000, 100), (2000, 200)]);

    let filter = InvoiceFilter {
        start_date: Some(3000),
        end_date: Some(9999),
        ..no_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_start_date_one_past_last_invoice_returns_empty() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(&env, &client, &admin, &[(1000, 100), (2000, 200)]);

    let filter = InvoiceFilter { start_date: Some(2001), ..no_filter() };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_end_date_one_before_first_invoice_returns_empty() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(&env, &client, &admin, &[(1000, 100), (2000, 200)]);

    let filter = InvoiceFilter { end_date: Some(999), ..no_filter() };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_full_range_spanning_all_invoices_returns_all() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(&env, &client, &admin, &[(1000, 100), (2000, 200), (3000, 300)]);

    let filter = InvoiceFilter {
        start_date: Some(1000),
        end_date: Some(3000),
        ..no_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 3);
}

// ---------------------------------------------------------------------------
// Date range combined with other filters
// ---------------------------------------------------------------------------

#[test]
fn test_date_range_combined_with_status_pending() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(&env, &client, &admin, &[(1000, 100), (2000, 200), (3000, 300)]);

    // All invoices are Pending; date range [1000, 2000] ? 2 results
    let filter = InvoiceFilter {
        status: Some(InvoiceStatus::Pending as u32),
        start_date: Some(1000),
        end_date: Some(2000),
        ..no_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|inv| inv.status == InvoiceStatus::Pending));
}

#[test]
fn test_date_range_combined_with_status_paid_returns_empty() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(&env, &client, &admin, &[(1000, 100), (2000, 200)]);

    // No invoices are Paid, so combining Paid status with any date range ? 0
    let filter = InvoiceFilter {
        status: Some(InvoiceStatus::Paid as u32),
        start_date: Some(1000),
        end_date: Some(2000),
        ..no_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_date_range_combined_with_min_amount() {
    let (env, client, _, admin) = setup_env();
    // Invoices: ts=1000 amount=100, ts=2000 amount=200, ts=3000 amount=300
    create_invoices_at(&env, &client, &admin, &[(1000, 100), (2000, 200), (3000, 300)]);

    // Date range [1000, 2000] AND min_amount=200 ? only invoice at ts=2000
    let filter = InvoiceFilter {
        min_amount: Some(200),
        start_date: Some(1000),
        end_date: Some(2000),
        ..no_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 1);
    assert_eq!(result.get(0).unwrap().amount, 200);
    assert_eq!(result.get(0).unwrap().date_created, 2000);
}

#[test]
fn test_date_range_combined_with_max_amount() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(&env, &client, &admin, &[(1000, 100), (2000, 200), (3000, 300)]);

    // Date range [2000, 3000] AND max_amount=200 ? only invoice at ts=2000
    let filter = InvoiceFilter {
        max_amount: Some(200),
        start_date: Some(2000),
        end_date: Some(3000),
        ..no_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 1);
    assert_eq!(result.get(0).unwrap().amount, 200);
}

#[test]
fn test_date_range_combined_with_amount_range() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(&env, &client, &admin, &[(1000, 100), (2000, 200), (3000, 300)]);

    // Date [1000, 3000] AND amount [150, 250] ? only invoice at ts=2000 (amount=200)
    let filter = InvoiceFilter {
        min_amount: Some(150),
        max_amount: Some(250),
        start_date: Some(1000),
        end_date: Some(3000),
        ..no_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 1);
    assert_eq!(result.get(0).unwrap().amount, 200);
}

#[test]
fn test_date_range_combined_with_merchant_filter() {
    let (env, client, _, admin) = setup_env();

    // Two merchants, each with invoices at different timestamps
    let merchant_a = Address::generate(&env);
    let merchant_b = Address::generate(&env);
    client.register_merchant(&merchant_a);
    client.register_merchant(&merchant_b);

    let token = make_token(&env, &admin);
    client.add_accepted_token(&admin, &token);

    env.ledger().set_timestamp(1000);
    client.create_invoice(&merchant_a, &String::from_str(&env, "A1"), &100, &token, &None);

    env.ledger().set_timestamp(2000);
    client.create_invoice(&merchant_b, &String::from_str(&env, "B1"), &200, &token, &None);

    env.ledger().set_timestamp(3000);
    client.create_invoice(&merchant_a, &String::from_str(&env, "A2"), &300, &token, &None);

    // Filter: merchant_a AND date range [1000, 2000] ? only A1
    let filter = InvoiceFilter {
        merchant: Some(merchant_a.clone()),
        start_date: Some(1000),
        end_date: Some(2000),
        ..no_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 1);
    assert_eq!(result.get(0).unwrap().date_created, 1000);
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_empty_invoice_list_with_date_filter_returns_empty() {
    let (_, client, _, _) = setup_env();

    let filter = InvoiceFilter {
        start_date: Some(1000),
        end_date: Some(9000),
        ..no_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_single_invoice_within_range() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(&env, &client, &admin, &[(5000, 100)]);

    let filter = InvoiceFilter {
        start_date: Some(4000),
        end_date: Some(6000),
        ..no_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 1);
    assert_eq!(result.get(0).unwrap().date_created, 5000);
}

#[test]
fn test_single_invoice_outside_range() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(&env, &client, &admin, &[(5000, 100)]);

    let filter = InvoiceFilter {
        start_date: Some(6000),
        end_date: Some(7000),
        ..no_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_many_invoices_only_middle_ones_match() {
    let (env, client, _, admin) = setup_env();
    create_invoices_at(
        &env,
        &client,
        &admin,
        &[
            (100, 100),
            (200, 100),
            (300, 100),
            (400, 100),
            (500, 100),
        ],
    );

    let filter = InvoiceFilter {
        start_date: Some(200),
        end_date: Some(400),
        ..no_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 3);
    assert_eq!(result.get(0).unwrap().date_created, 200);
    assert_eq!(result.get(1).unwrap().date_created, 300);
    assert_eq!(result.get(2).unwrap().date_created, 400);
}
