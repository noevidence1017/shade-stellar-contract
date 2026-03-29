#![cfg(test)]

use crate::shade::{Shade, ShadeClient};
use crate::types::{InvoiceFilter, InvoiceStatus, MerchantFilter};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env, String};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup_test() -> (Env, ShadeClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, contract_id, admin)
}

fn create_token(env: &Env) -> Address {
    env.register_stellar_asset_contract_v2(Address::generate(env))
        .address()
}

fn no_merchant_filter() -> MerchantFilter {
    MerchantFilter {
        is_active: None,
        is_verified: None,
    }
}

fn no_invoice_filter() -> InvoiceFilter {
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
// Merchant query – active filter
// ---------------------------------------------------------------------------

#[test]
fn test_get_merchants_filter_active_only() {
    let (env, client, _contract_id, admin) = setup_test();

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    let m3 = Address::generate(&env);

    client.register_merchant(&m1);
    client.register_merchant(&m2);
    client.register_merchant(&m3);

    // deactivate merchant 2
    client.set_merchant_status(&admin, &2u64, &false);

    let filter = MerchantFilter {
        is_active: Some(true),
        is_verified: None,
    };
    let result = client.get_merchants(&filter);
    assert_eq!(result.len(), 2);
    for m in result.iter() {
        assert!(m.active);
    }
}

#[test]
fn test_get_merchants_filter_inactive_only() {
    let (env, client, _contract_id, admin) = setup_test();

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);

    client.register_merchant(&m1);
    client.register_merchant(&m2);

    client.set_merchant_status(&admin, &1u64, &false);
    client.set_merchant_status(&admin, &2u64, &false);

    let filter = MerchantFilter {
        is_active: Some(false),
        is_verified: None,
    };
    let result = client.get_merchants(&filter);
    assert_eq!(result.len(), 2);
    for m in result.iter() {
        assert!(!m.active);
    }
}

// ---------------------------------------------------------------------------
// Merchant query – verified filter
// ---------------------------------------------------------------------------

#[test]
fn test_get_merchants_filter_verified_only() {
    let (env, client, _contract_id, admin) = setup_test();

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    let m3 = Address::generate(&env);

    client.register_merchant(&m1);
    client.register_merchant(&m2);
    client.register_merchant(&m3);

    // verify only merchant 1
    client.verify_merchant(&admin, &1u64, &true);

    let filter = MerchantFilter {
        is_active: None,
        is_verified: Some(true),
    };
    let result = client.get_merchants(&filter);
    assert_eq!(result.len(), 1);
    assert_eq!(result.get(0).unwrap().id, 1);
    assert!(result.get(0).unwrap().verified);
}

#[test]
fn test_get_merchants_filter_unverified_only() {
    let (env, client, _contract_id, admin) = setup_test();

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);

    client.register_merchant(&m1);
    client.register_merchant(&m2);

    client.verify_merchant(&admin, &1u64, &true);

    let filter = MerchantFilter {
        is_active: None,
        is_verified: Some(false),
    };
    let result = client.get_merchants(&filter);
    assert_eq!(result.len(), 1);
    assert_eq!(result.get(0).unwrap().id, 2);
    assert!(!result.get(0).unwrap().verified);
}

// ---------------------------------------------------------------------------
// Merchant query – combined active + verified filter
// ---------------------------------------------------------------------------

#[test]
fn test_get_merchants_filter_active_and_verified() {
    let (env, client, _contract_id, admin) = setup_test();

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    let m3 = Address::generate(&env);
    let m4 = Address::generate(&env);

    client.register_merchant(&m1); // active + verified
    client.register_merchant(&m2); // inactive + verified
    client.register_merchant(&m3); // active + unverified
    client.register_merchant(&m4); // inactive + unverified

    client.verify_merchant(&admin, &1u64, &true);
    client.verify_merchant(&admin, &2u64, &true);
    client.set_merchant_status(&admin, &2u64, &false);
    client.set_merchant_status(&admin, &4u64, &false);

    let filter = MerchantFilter {
        is_active: Some(true),
        is_verified: Some(true),
    };
    let result = client.get_merchants(&filter);
    assert_eq!(result.len(), 1);
    assert_eq!(result.get(0).unwrap().id, 1);
    assert!(result.get(0).unwrap().active);
    assert!(result.get(0).unwrap().verified);
}

#[test]
fn test_get_merchants_filter_active_and_unverified() {
    let (env, client, _contract_id, admin) = setup_test();

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    let m3 = Address::generate(&env);

    client.register_merchant(&m1); // active, unverified
    client.register_merchant(&m2); // active, verified
    client.register_merchant(&m3); // inactive, unverified

    client.verify_merchant(&admin, &2u64, &true);
    client.set_merchant_status(&admin, &3u64, &false);

    let filter = MerchantFilter {
        is_active: Some(true),
        is_verified: Some(false),
    };
    let result = client.get_merchants(&filter);
    assert_eq!(result.len(), 1);
    assert_eq!(result.get(0).unwrap().id, 1);
}

#[test]
fn test_get_merchants_no_filter_returns_all() {
    let (env, client, _contract_id, _admin) = setup_test();

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    let m3 = Address::generate(&env);

    client.register_merchant(&m1);
    client.register_merchant(&m2);
    client.register_merchant(&m3);

    let result = client.get_merchants(&no_merchant_filter());
    assert_eq!(result.len(), 3);
}

// ---------------------------------------------------------------------------
// Invoice query – merchant filter
// ---------------------------------------------------------------------------

#[test]
fn test_get_invoices_filter_by_merchant() {
    let (env, client, _contract_id, admin) = setup_test();

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    client.register_merchant(&m1);
    client.register_merchant(&m2);

    let token = create_token(&env);
    client.add_accepted_token(&admin, &token);

    client.create_invoice(&m1, &String::from_str(&env, "Inv A"), &100, &token, &None);
    client.create_invoice(&m1, &String::from_str(&env, "Inv B"), &200, &token, &None);
    client.create_invoice(&m2, &String::from_str(&env, "Inv C"), &300, &token, &None);

    let filter = InvoiceFilter {
        merchant: Some(m1.clone()),
        ..no_invoice_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 2);
    for inv in result.iter() {
        assert_eq!(inv.merchant_id, 1);
    }
}

// ---------------------------------------------------------------------------
// Invoice query – status filter
// ---------------------------------------------------------------------------

#[test]
fn test_get_invoices_filter_by_status_pending() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = create_token(&env);
    client.add_accepted_token(&admin, &token);

    client.create_invoice(&merchant, &String::from_str(&env, "Inv 1"), &100, &token, &None);
    client.create_invoice(&merchant, &String::from_str(&env, "Inv 2"), &200, &token, &None);

    let filter = InvoiceFilter {
        status: Some(InvoiceStatus::Pending as u32),
        ..no_invoice_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 2);
    for inv in result.iter() {
        assert_eq!(inv.status, InvoiceStatus::Pending);
    }
}

#[test]
fn test_get_invoices_filter_by_status_cancelled() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = create_token(&env);
    client.add_accepted_token(&admin, &token);

    let id1 = client.create_invoice(&merchant, &String::from_str(&env, "Inv 1"), &100, &token, &None);
    client.create_invoice(&merchant, &String::from_str(&env, "Inv 2"), &200, &token, &None);

    client.void_invoice(&merchant, &id1);

    let filter = InvoiceFilter {
        status: Some(InvoiceStatus::Cancelled as u32),
        ..no_invoice_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 1);
    assert_eq!(result.get(0).unwrap().id, id1);
    assert_eq!(result.get(0).unwrap().status, InvoiceStatus::Cancelled);
}

// ---------------------------------------------------------------------------
// Invoice query – min/max amount filter
// ---------------------------------------------------------------------------

#[test]
fn test_get_invoices_filter_by_min_amount() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = create_token(&env);
    client.add_accepted_token(&admin, &token);

    client.create_invoice(&merchant, &String::from_str(&env, "Cheap"), &50, &token, &None);
    client.create_invoice(&merchant, &String::from_str(&env, "Mid"), &150, &token, &None);
    client.create_invoice(&merchant, &String::from_str(&env, "Expensive"), &500, &token, &None);

    let filter = InvoiceFilter {
        min_amount: Some(150),
        ..no_invoice_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 2);
    for inv in result.iter() {
        assert!(inv.amount >= 150);
    }
}

#[test]
fn test_get_invoices_filter_by_max_amount() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = create_token(&env);
    client.add_accepted_token(&admin, &token);

    client.create_invoice(&merchant, &String::from_str(&env, "Cheap"), &50, &token, &None);
    client.create_invoice(&merchant, &String::from_str(&env, "Mid"), &150, &token, &None);
    client.create_invoice(&merchant, &String::from_str(&env, "Expensive"), &500, &token, &None);

    let filter = InvoiceFilter {
        max_amount: Some(150),
        ..no_invoice_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 2);
    for inv in result.iter() {
        assert!(inv.amount <= 150);
    }
}

#[test]
fn test_get_invoices_filter_by_amount_range() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = create_token(&env);
    client.add_accepted_token(&admin, &token);

    client.create_invoice(&merchant, &String::from_str(&env, "10"), &10, &token, &None);
    client.create_invoice(&merchant, &String::from_str(&env, "100"), &100, &token, &None);
    client.create_invoice(&merchant, &String::from_str(&env, "200"), &200, &token, &None);
    client.create_invoice(&merchant, &String::from_str(&env, "500"), &500, &token, &None);

    let filter = InvoiceFilter {
        min_amount: Some(100),
        max_amount: Some(200),
        ..no_invoice_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 2);
    for inv in result.iter() {
        assert!(inv.amount >= 100 && inv.amount <= 200);
    }
}

// ---------------------------------------------------------------------------
// Invoice query – combined filters
// ---------------------------------------------------------------------------

#[test]
fn test_get_invoices_combined_merchant_and_status() {
    let (env, client, _contract_id, admin) = setup_test();

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    client.register_merchant(&m1);
    client.register_merchant(&m2);

    let token = create_token(&env);
    client.add_accepted_token(&admin, &token);

    let id1 = client.create_invoice(&m1, &String::from_str(&env, "M1 Active"), &100, &token, &None);
    client.create_invoice(&m1, &String::from_str(&env, "M1 Voided"), &200, &token, &None);
    client.create_invoice(&m2, &String::from_str(&env, "M2 Active"), &300, &token, &None);

    // void invoice 2 (id = 2)
    client.void_invoice(&m1, &2u64);

    let filter = InvoiceFilter {
        status: Some(InvoiceStatus::Pending as u32),
        merchant: Some(m1.clone()),
        ..no_invoice_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 1);
    assert_eq!(result.get(0).unwrap().id, id1);
    assert_eq!(result.get(0).unwrap().status, InvoiceStatus::Pending);
    assert_eq!(result.get(0).unwrap().merchant_id, 1);
}

#[test]
fn test_get_invoices_combined_merchant_and_amount_range() {
    let (env, client, _contract_id, admin) = setup_test();

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    client.register_merchant(&m1);
    client.register_merchant(&m2);

    let token = create_token(&env);
    client.add_accepted_token(&admin, &token);

    client.create_invoice(&m1, &String::from_str(&env, "M1 small"), &50, &token, &None);
    client.create_invoice(&m1, &String::from_str(&env, "M1 large"), &1000, &token, &None);
    client.create_invoice(&m2, &String::from_str(&env, "M2 large"), &1000, &token, &None);

    let filter = InvoiceFilter {
        merchant: Some(m1.clone()),
        min_amount: Some(100),
        max_amount: Some(5000),
        ..no_invoice_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 1);
    assert_eq!(result.get(0).unwrap().amount, 1000);
    assert_eq!(result.get(0).unwrap().merchant_id, 1);
}

#[test]
fn test_get_invoices_combined_status_and_amount() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = create_token(&env);
    client.add_accepted_token(&admin, &token);

    let id_small = client.create_invoice(&merchant, &String::from_str(&env, "Small"), &50, &token, &None);
    client.create_invoice(&merchant, &String::from_str(&env, "Large"), &500, &token, &None);

    // void small invoice
    client.void_invoice(&merchant, &id_small);

    // pending + amount >= 100
    let filter = InvoiceFilter {
        status: Some(InvoiceStatus::Pending as u32),
        min_amount: Some(100),
        ..no_invoice_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 1);
    assert_eq!(result.get(0).unwrap().amount, 500);
    assert_eq!(result.get(0).unwrap().status, InvoiceStatus::Pending);
}

// ---------------------------------------------------------------------------
// Edge – empty result cases
// ---------------------------------------------------------------------------

#[test]
fn test_get_merchants_empty_when_none_registered() {
    let (_env, client, _contract_id, _admin) = setup_test();

    let result = client.get_merchants(&no_merchant_filter());
    assert_eq!(result.len(), 0);
}

#[test]
fn test_get_merchants_active_filter_no_match() {
    let (env, client, _contract_id, admin) = setup_test();

    let m1 = Address::generate(&env);
    client.register_merchant(&m1);
    // deactivate the only merchant
    client.set_merchant_status(&admin, &1u64, &false);

    let filter = MerchantFilter {
        is_active: Some(true),
        is_verified: None,
    };
    let result = client.get_merchants(&filter);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_get_merchants_verified_filter_no_match() {
    let (env, client, _contract_id, _admin) = setup_test();

    let m1 = Address::generate(&env);
    client.register_merchant(&m1);
    // merchant is registered but never verified

    let filter = MerchantFilter {
        is_active: None,
        is_verified: Some(true),
    };
    let result = client.get_merchants(&filter);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_get_merchants_combined_filter_no_match() {
    let (env, client, _contract_id, _admin) = setup_test();

    let m1 = Address::generate(&env);
    client.register_merchant(&m1);
    // active=true, verified=false → querying active+verified gives 0

    let filter = MerchantFilter {
        is_active: Some(true),
        is_verified: Some(true),
    };
    let result = client.get_merchants(&filter);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_get_invoices_empty_when_none_created() {
    let (_env, client, _contract_id, _admin) = setup_test();

    let result = client.get_invoices(&no_invoice_filter());
    assert_eq!(result.len(), 0);
}

#[test]
fn test_get_invoices_merchant_filter_no_match() {
    let (env, client, _contract_id, admin) = setup_test();

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    client.register_merchant(&m1);
    client.register_merchant(&m2);

    let token = create_token(&env);
    client.add_accepted_token(&admin, &token);

    // only m1 has invoices
    client.create_invoice(&m1, &String::from_str(&env, "Inv"), &100, &token, &None);

    let filter = InvoiceFilter {
        merchant: Some(m2.clone()),
        ..no_invoice_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_get_invoices_status_filter_no_match() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = create_token(&env);
    client.add_accepted_token(&admin, &token);

    client.create_invoice(&merchant, &String::from_str(&env, "Inv"), &100, &token, &None);
    // invoice is Pending; querying Paid returns nothing

    let filter = InvoiceFilter {
        status: Some(InvoiceStatus::Paid as u32),
        ..no_invoice_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_get_invoices_amount_range_no_match() {
    let (env, client, _contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = create_token(&env);
    client.add_accepted_token(&admin, &token);

    client.create_invoice(&merchant, &String::from_str(&env, "Inv"), &100, &token, &None);

    // min > max of existing invoices
    let filter = InvoiceFilter {
        min_amount: Some(200),
        max_amount: Some(500),
        ..no_invoice_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_get_invoices_combined_filters_no_match() {
    let (env, client, _contract_id, admin) = setup_test();

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    client.register_merchant(&m1);
    client.register_merchant(&m2);

    let token = create_token(&env);
    client.add_accepted_token(&admin, &token);

    client.create_invoice(&m1, &String::from_str(&env, "M1 Inv"), &100, &token, &None);

    // filter by m2 + high min amount — both conditions exclude the only invoice
    let filter = InvoiceFilter {
        merchant: Some(m2.clone()),
        min_amount: Some(500),
        ..no_invoice_filter()
    };
    let result = client.get_invoices(&filter);
    assert_eq!(result.len(), 0);
}
