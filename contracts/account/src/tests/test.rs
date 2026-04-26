#![cfg(test)]

use crate::account::MerchantAccount;
use crate::account::MerchantAccountClient;
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{Address, Env};

#[test]
fn test_initialize() {
    let env = Env::default();
    let contract_id = env.register(MerchantAccount, ());
    let client = MerchantAccountClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let manager = Address::generate(&env);
    let merchant_id = 1;
    client.initialize(&merchant, &manager, &merchant_id);
    assert_eq!(client.get_merchant(), merchant);
}

#[should_panic(expected = "HostError: Error(Contract, #1)")]
#[test]
fn test_initialize_twice() {
    let env = Env::default();
    let contract_id = env.register(MerchantAccount, ());
    let client = MerchantAccountClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let manager = Address::generate(&env);
    let merchant_id = 1;
    client.initialize(&merchant, &manager, &merchant_id);
    client.initialize(&merchant, &manager, &merchant_id);
}

#[should_panic(expected = "HostError: Error(Contract, #2)")]
#[test]
fn test_get_merchant_not_initialized() {
    let env = Env::default();
    let contract_id = env.register(MerchantAccount, ());
    let client = MerchantAccountClient::new(&env, &contract_id);

    client.get_merchant();
}

#[test]
fn test_verify_account() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(MerchantAccount, ());
    let client = MerchantAccountClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let manager = Address::generate(&env);
    let merchant_id = 1;
    client.initialize(&merchant, &manager, &merchant_id);

    assert!(!client.is_verified_account());

    client.verify_account();
    let events = env.events().all();

    assert!(client.is_verified_account());

    assert!(
        !events.is_empty(),
        "No events captured immediately after verify_account!"
    );
    let (_event_contract_id, _topics, _data) = events.get(events.len() - 1).unwrap();
}

#[should_panic(expected = "HostError: Error(Auth, InvalidAction)")]
#[test]
fn test_verify_account_unauthorized() {
    let env = Env::default();
    // No mock_all_auths here to test auth failure
    let contract_id = env.register(MerchantAccount, ());
    let client = MerchantAccountClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let manager = Address::generate(&env);
    let merchant_id = 1;
    client.initialize(&merchant, &manager, &merchant_id);

    // This should fail because we're not authenticated as manager
    client.verify_account();
}

#[test]
fn test_restrict_account() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(MerchantAccount, ());
    let client = MerchantAccountClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let manager = Address::generate(&env);
    let merchant_id = 1;
    client.initialize(&merchant, &manager, &merchant_id);

    assert!(!client.is_restricted_account());

    client.restrict_account(&true);
    let events = env.events().all();

    assert!(client.is_restricted_account());

    client.restrict_account(&false);
    assert!(!client.is_restricted_account());

    assert!(
        !events.is_empty(),
        "No events captured immediately after restrict_account!"
    );
}

#[should_panic(expected = "HostError: Error(Auth, InvalidAction)")]
#[test]
fn test_restrict_account_unauthorized() {
    let env = Env::default();
    let contract_id = env.register(MerchantAccount, ());
    let client = MerchantAccountClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let manager = Address::generate(&env);
    let merchant_id = 1;
    client.initialize(&merchant, &manager, &merchant_id);

    // This should fail because we're not authenticated as manager
    client.restrict_account(&true);
}
