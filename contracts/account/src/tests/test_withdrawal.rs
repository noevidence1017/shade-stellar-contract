#![cfg(test)]

use crate::account::MerchantAccount;
use crate::account::MerchantAccountClient;
use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _};
use soroban_sdk::{token, Address, Env};

fn setup_initialized_account(env: &Env) -> (Address, MerchantAccountClient<'_>, Address) {
    let contract_id = env.register(MerchantAccount, ());
    let client = MerchantAccountClient::new(env, &contract_id);

    let merchant = Address::generate(env);
    let manager = Address::generate(env);
    let merchant_id = 1u64;
    client.initialize(&merchant, &manager, &merchant_id);

    (contract_id, client, merchant)
}

fn create_test_token(env: &Env) -> Address {
    let token_admin = Address::generate(env);
    env.register_stellar_asset_contract_v2(token_admin)
        .address()
}

/// Test Case 1: Successful Withdrawal
/// Merchant successfully withdraws funds from their account.
#[test]
fn test_withdraw_to_success_with_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let (_contract_id, client, _merchant) = setup_initialized_account(&env);
    let token = create_test_token(&env);
    let recipient = Address::generate(&env);

    // Setup balance in merchant account
    let token_client = token::StellarAssetClient::new(&env, &token);
    let contract_address = _contract_id.clone();
    token_client.mint(&contract_address, &5000);

    // Get balance before withdrawal
    let balance_before = client.get_balance(&token);
    assert_eq!(balance_before, 5000, "Initial balance should be 5000");

    // Withdraw funds
    client.withdraw_to(&token, &3000, &recipient);

    // Verify balance after withdrawal
    let balance_after = client.get_balance(&token);
    assert_eq!(
        balance_after, 2000,
        "Balance after withdrawal should be 2000"
    );

    // Verify recipient received funds
    let recipient_balance = token_client.balance(&recipient);
    assert_eq!(
        recipient_balance, 3000,
        "Recipient should have received 3000"
    );
}

/// Test Case 2: Requires Merchant Authentication
/// Only the merchant can call withdraw_to. Unauthorized calls must panic.
#[test]
#[should_panic(expected = "HostError: Error(Auth, InvalidAction)")]
fn test_withdraw_to_requires_merchant_auth() {
    let env = Env::default();
    let contract_id = env.register(MerchantAccount, ());
    let client = MerchantAccountClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let manager = Address::generate(&env);
    let merchant_id = 1u64;

    client.initialize(&merchant, &manager, &merchant_id);

    let recipient = Address::generate(&env);
    let token = create_test_token(&env);

    // Attempt withdrawal without merchant authentication (should panic)
    client.withdraw_to(&token, &500_000i128, &recipient);
}

/// Test Case 3: Insufficient Balance Validation
/// Attempting to withdraw more than available balance should panic.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #4)")]
fn test_withdraw_to_insufficient_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let (_contract_id, client, _merchant) = setup_initialized_account(&env);
    let token = create_test_token(&env);
    let recipient = Address::generate(&env);

    // Try to withdraw more than available
    let current_balance = client.get_balance(&token);
    assert_eq!(current_balance, 0, "Initial balance should be 0");

    client.withdraw_to(&token, &1000, &recipient);
}

/// Test Case 4: Withdrawing Exact Balance
/// Withdraw exactly the total balance available.
#[test]
fn test_withdraw_to_exact_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let (_contract_id, client, _merchant) = setup_initialized_account(&env);
    let token = create_test_token(&env);
    let recipient = Address::generate(&env);

    // Setup balance
    let token_client = token::StellarAssetClient::new(&env, &token);
    let contract_address = _contract_id.clone();
    token_client.mint(&contract_address, &1000);

    // Withdraw exact balance
    client.withdraw_to(&token, &1000, &recipient);

    // Verify balance is now zero
    let balance_after = client.get_balance(&token);
    assert_eq!(
        balance_after, 0,
        "Balance after exact withdrawal should be 0"
    );

    // Verify recipient received all funds
    let recipient_balance = token_client.balance(&recipient);
    assert_eq!(
        recipient_balance, 1000,
        "Recipient should have received 1000"
    );
}

/// Test Case 5: Multiple Withdrawals
/// Verify that multiple withdrawals work correctly and reduce balance properly.
#[test]
fn test_withdraw_to_multiple_withdrawals() {
    let env = Env::default();
    env.mock_all_auths();

    let (_contract_id, client, _merchant) = setup_initialized_account(&env);
    let token = create_test_token(&env);

    let token_client = token::StellarAssetClient::new(&env, &token);
    let contract_address = _contract_id.clone();
    token_client.mint(&contract_address, &10000);

    let recipient1 = Address::generate(&env);
    let recipient2 = Address::generate(&env);
    let recipient3 = Address::generate(&env);

    // First withdrawal
    client.withdraw_to(&token, &2000, &recipient1);
    let balance_after_first = client.get_balance(&token);
    assert_eq!(balance_after_first, 8000);

    // Second withdrawal
    client.withdraw_to(&token, &3000, &recipient2);
    let balance_after_second = client.get_balance(&token);
    assert_eq!(balance_after_second, 5000);

    // Third withdrawal
    client.withdraw_to(&token, &5000, &recipient3);
    let balance_after_third = client.get_balance(&token);
    assert_eq!(balance_after_third, 0);

    // Verify all recipients got their funds
    assert_eq!(token_client.balance(&recipient1), 2000);
    assert_eq!(token_client.balance(&recipient2), 3000);
    assert_eq!(token_client.balance(&recipient3), 5000);
}

/// Test Case 6: Zero Amount Withdrawal
/// Attempting to withdraw zero amount should succeed but transfer nothing.
#[test]
fn test_withdraw_to_zero_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let (_contract_id, client, _merchant) = setup_initialized_account(&env);
    let token = create_test_token(&env);
    let recipient = Address::generate(&env);

    let token_client = token::StellarAssetClient::new(&env, &token);
    let contract_address = _contract_id.clone();
    token_client.mint(&contract_address, &5000);

    // Withdraw zero
    client.withdraw_to(&token, &0, &recipient);

    // Verify balance unchanged
    let balance = client.get_balance(&token);
    assert_eq!(
        balance, 5000,
        "Balance should remain unchanged after zero withdrawal"
    );

    // Verify recipient received nothing
    let recipient_balance = token_client.balance(&recipient);
    assert_eq!(
        recipient_balance, 0,
        "Recipient should have received nothing"
    );
}

/// Test Case 7: Withdrawal to Same Address
/// Withdraw to the merchant's own address (edge case).
#[test]
fn test_withdraw_to_same_address() {
    let env = Env::default();
    env.mock_all_auths();

    let (_contract_id, client, merchant) = setup_initialized_account(&env);
    let token = create_test_token(&env);

    let token_client = token::StellarAssetClient::new(&env, &token);
    let contract_address = _contract_id.clone();
    token_client.mint(&contract_address, &5000);

    // Withdraw to merchant's own address
    client.withdraw_to(&token, &3000, &merchant);

    // Verify balance decreased
    let balance_after = client.get_balance(&token);
    assert_eq!(balance_after, 2000);

    // Verify merchant received funds
    let merchant_balance = token_client.balance(&merchant);
    assert_eq!(merchant_balance, 3000);
}

/// Test Case 8: Withdrawal Event Emission
/// Verify that withdrawal events are properly emitted.
#[test]
fn test_withdraw_to_emits_event() {
    let env = Env::default();
    env.mock_all_auths();

    let (_contract_id, client, _merchant) = setup_initialized_account(&env);
    let token = create_test_token(&env);
    let recipient = Address::generate(&env);

    let token_client = token::StellarAssetClient::new(&env, &token);
    let contract_address = _contract_id.clone();
    token_client.mint(&contract_address, &5000);

    // Clear events before withdrawal
    env.events().all();

    // Perform withdrawal
    client.withdraw_to(&token, &2500, &recipient);

    // Verify events were emitted
    let events = env.events().all();
    assert!(!events.is_empty(), "Withdrawal event should be emitted");
}

/// Test Case 9: Withdrawal Analytics Aggregation
/// Verify that withdrawal counts and amounts are tracked per token.
#[test]
fn test_withdraw_to_tracks_analytics() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);

    let (_contract_id, client, _merchant) = setup_initialized_account(&env);
    let token = create_test_token(&env);

    let token_client = token::StellarAssetClient::new(&env, &token);
    let contract_address = _contract_id.clone();
    token_client.mint(&contract_address, &10000);

    let recipient1 = Address::generate(&env);
    let recipient2 = Address::generate(&env);

    client.withdraw_to(&token, &2500, &recipient1);
    client.withdraw_to(&token, &1500, &recipient2);

    let analytics = client.get_withdrawal_analytics(&token);
    assert_eq!(analytics.token, token);
    assert_eq!(analytics.total_withdrawn, 4000);
    assert_eq!(analytics.withdrawal_count, 2);
    assert_eq!(analytics.last_withdrawn_at, 1_000_000);
}
