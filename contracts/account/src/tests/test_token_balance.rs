#![cfg(test)]

use crate::account::MerchantAccount;
use crate::account::MerchantAccountClient;
use crate::events::{RefundProcessedEvent, TokenAddedEvent};
use crate::types::{DataKey, TokenBalance};
use soroban_sdk::events::Event;
use soroban_sdk::testutils::{Address as _, Events as _, MockAuth, MockAuthInvoke};
use soroban_sdk::{token, Address, Env, IntoVal, Map, Symbol, TryFromVal, Val};

fn setup_initialized_account(env: &Env) -> (Address, MerchantAccountClient<'_>, Address) {
    let contract_id = env.register(MerchantAccount, ());
    let client = MerchantAccountClient::new(env, &contract_id);

    let merchant = Address::generate(env);
    let manager = Address::generate(env);
    let merchant_id = 1;
    client.initialize(&merchant, &manager, &merchant_id);

    (contract_id, client, manager)
}

fn create_test_token(env: &Env) -> Address {
    let token_admin = Address::generate(env);
    env.register_stellar_asset_contract_v2(token_admin)
        .address()
}

#[test]
fn test_add_token_tracks_token_and_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client, _) = setup_initialized_account(&env);

    let token = create_test_token(&env);
    client.add_token(&token);
    let events = env.events().all();
    assert_eq!(events.len(), 1);

    let expected_event = TokenAddedEvent {
        token: token.clone(),
        timestamp: env.ledger().timestamp(),
    };
    let emitted = events.get(events.len() - 1).unwrap();
    let expected_data_val = expected_event.data(&env);
    let emitted_data = Map::<Symbol, Val>::try_from_val(&env, &emitted.2).unwrap();
    let expected_data = Map::<Symbol, Val>::try_from_val(&env, &expected_data_val).unwrap();
    assert_eq!(emitted.0, contract_id);
    assert_eq!(emitted.1, expected_event.topics(&env));
    assert_eq!(emitted_data, expected_data);

    assert!(client.has_token(&token));
}

#[test]
fn test_add_token_duplicate_is_idempotent() {
    let env = Env::default();
    env.mock_all_auths();
    let (_, client, _) = setup_initialized_account(&env);

    let token = create_test_token(&env);
    client.add_token(&token);
    let first_add_events = env.events().all();
    assert_eq!(first_add_events.len(), 1);

    client.add_token(&token);
    let second_add_events = env.events().all();
    assert_eq!(second_add_events.len(), 0);

    let balances = client.get_balances();
    assert_eq!(balances.len(), 1);
    assert_eq!(
        balances.get(0).unwrap(),
        TokenBalance {
            token: token.clone(),
            balance: 0,
        }
    );
}

#[test]
#[should_panic]
fn test_add_token_unauthorized_access_panics() {
    let env = Env::default();
    let (contract_id, client, _) = setup_initialized_account(&env);
    let token = create_test_token(&env);
    let random = Address::generate(&env);

    client
        .mock_auths(&[MockAuth {
            address: &random,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "add_token",
                args: (&token,).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .add_token(&token);
}

#[test]
fn test_get_balance_returns_minted_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client, _) = setup_initialized_account(&env);

    let token = create_test_token(&env);
    client.add_token(&token);

    let token_admin_client = token::StellarAssetClient::new(&env, &token);
    let minted_amount = 1_500_i128;
    token_admin_client.mint(&contract_id, &minted_amount);

    let balance = client.get_balance(&token);
    assert_eq!(balance, minted_amount);
}

#[test]
fn test_get_balances_returns_all_tracked_token_balances() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client, _) = setup_initialized_account(&env);

    let token_a = create_test_token(&env);
    let token_b = create_test_token(&env);
    client.add_token(&token_a);
    client.add_token(&token_b);

    let token_a_admin_client = token::StellarAssetClient::new(&env, &token_a);
    let token_b_admin_client = token::StellarAssetClient::new(&env, &token_b);
    let amount_a = 42_i128;
    let amount_b = 99_i128;
    token_a_admin_client.mint(&contract_id, &amount_a);
    token_b_admin_client.mint(&contract_id, &amount_b);

    let balances = client.get_balances();
    assert_eq!(balances.len(), 2);

    let mut saw_a = false;
    let mut saw_b = false;
    for token_balance in balances.iter() {
        if token_balance.token == token_a {
            assert_eq!(token_balance.balance, amount_a);
            saw_a = true;
        } else if token_balance.token == token_b {
            assert_eq!(token_balance.balance, amount_b);
            saw_b = true;
        }
    }

    assert!(saw_a);
    assert!(saw_b);
}

#[test]
fn test_has_token_returns_false_for_untracked_token() {
    let env = Env::default();
    let (_, client, _) = setup_initialized_account(&env);

    let untracked_token = create_test_token(&env);
    assert!(!client.has_token(&untracked_token));
}

#[test]
fn test_refund_transfers_tokens_and_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client, _) = setup_initialized_account(&env);

    let token = create_test_token(&env);
    let recipient = Address::generate(&env);
    let refund_amount = 275_i128;
    let initial_balance = 1_000_i128;

    let token_admin_client = token::StellarAssetClient::new(&env, &token);
    let token_client = token::TokenClient::new(&env, &token);
    token_admin_client.mint(&contract_id, &initial_balance);

    client.refund(&token, &refund_amount, &recipient);

    let events = env.events().all();
    assert!(!events.is_empty());

    let expected_event = RefundProcessedEvent {
        token: token.clone(),
        amount: refund_amount,
        recipient: recipient.clone(),
        timestamp: env.ledger().timestamp(),
    };
    let emitted = events.get(events.len() - 1).unwrap();
    let expected_data_val = expected_event.data(&env);
    let emitted_data = Map::<Symbol, Val>::try_from_val(&env, &emitted.2).unwrap();
    let expected_data = Map::<Symbol, Val>::try_from_val(&env, &expected_data_val).unwrap();
    assert_eq!(emitted.0, contract_id.clone());
    assert_eq!(emitted.1, expected_event.topics(&env));
    assert_eq!(emitted_data, expected_data);

    assert_eq!(
        token_client.balance(&contract_id),
        initial_balance - refund_amount
    );
    assert_eq!(token_client.balance(&recipient), refund_amount);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #5)")]
fn test_refund_panics_when_account_is_restricted() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client, _) = setup_initialized_account(&env);

    env.as_contract(&contract_id, || {
        env.storage().persistent().set(&DataKey::Restricted, &true);
    });

    let token = create_test_token(&env);
    let recipient = Address::generate(&env);
    client.refund(&token, &10_i128, &recipient);
}

#[test]
#[should_panic]
fn test_refund_unauthorized_access_panics() {
    let env = Env::default();
    let (contract_id, client, _) = setup_initialized_account(&env);
    let token = create_test_token(&env);
    let recipient = Address::generate(&env);
    let random = Address::generate(&env);
    let amount = 10_i128;

    client
        .mock_auths(&[MockAuth {
            address: &random,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "refund",
                args: (&token, &amount, &recipient).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .refund(&token, &amount, &recipient);
}
