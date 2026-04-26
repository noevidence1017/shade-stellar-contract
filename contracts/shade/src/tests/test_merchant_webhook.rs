#![cfg(test)]

use crate::shade::{Shade, ShadeClient};
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{Address, Env, Map, String, Symbol, TryIntoVal, Val};

fn setup_test() -> (Env, ShadeClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, contract_id, admin)
}

#[test]
fn test_merchant_webhook_defaults_to_empty() {
    let (env, client, _contract_id, _admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let webhook = client.get_merchant_webhook(&1u64);
    assert_eq!(webhook, String::from_str(&env, ""));
}

#[test]
fn test_set_merchant_webhook_success() {
    let (env, client, contract_id, _admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let webhook = String::from_str(&env, "https://relay.example.com/hook");
    client.set_merchant_webhook(&merchant, &webhook);
    let events = env.events().all();

    assert!(!events.is_empty(), "No events captured after webhook set");

    let (event_contract_id, _topics, data) = events.get(events.len() - 1).unwrap();
    assert_eq!(event_contract_id, contract_id.clone());

    let data_map: Map<Symbol, Val> = data.try_into_val(&env).expect("Data should be a Map");

    let merchant_val = data_map
        .get(Symbol::new(&env, "merchant"))
        .expect("Should have merchant field");
    let merchant_id_val = data_map
        .get(Symbol::new(&env, "merchant_id"))
        .expect("Should have merchant_id field");
    let webhook_val = data_map
        .get(Symbol::new(&env, "webhook"))
        .expect("Should have webhook field");

    let merchant_in_event: Address = merchant_val.try_into_val(&env).unwrap();
    let merchant_id_in_event: u64 = merchant_id_val.try_into_val(&env).unwrap();
    let webhook_in_event: String = webhook_val.try_into_val(&env).unwrap();

    assert_eq!(merchant_in_event, merchant.clone());
    assert_eq!(merchant_id_in_event, 1u64);
    assert_eq!(webhook_in_event, webhook.clone());

    assert_eq!(client.get_merchant_webhook(&1u64), webhook);

    let merchant_data = client.get_merchant(&1u64);
    assert_eq!(merchant_data.webhook, webhook);
}

#[test]
fn test_update_merchant_webhook() {
    let (env, client, _contract_id, _admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let webhook1 = String::from_str(&env, "https://relay.example.com/hook");
    client.set_merchant_webhook(&merchant, &webhook1);
    assert_eq!(client.get_merchant_webhook(&1u64), webhook1);

    let webhook2 = String::from_str(&env, "https://new.example.com/v2/hook");
    client.set_merchant_webhook(&merchant, &webhook2);
    assert_eq!(client.get_merchant_webhook(&1u64), webhook2);
}

#[test]
fn test_clear_merchant_webhook() {
    let (env, client, _contract_id, _admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let webhook = String::from_str(&env, "https://relay.example.com/hook");
    client.set_merchant_webhook(&merchant, &webhook);
    assert_eq!(client.get_merchant_webhook(&1u64), webhook);

    let empty = String::from_str(&env, "");
    client.set_merchant_webhook(&merchant, &empty);
    assert_eq!(client.get_merchant_webhook(&1u64), empty);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #6)")]
fn test_set_webhook_for_unregistered_merchant_fails() {
    let (env, client, _contract_id, _admin) = setup_test();

    let unregistered = Address::generate(&env);
    let webhook = String::from_str(&env, "https://relay.example.com/hook");
    client.set_merchant_webhook(&unregistered, &webhook);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #6)")]
fn test_get_webhook_for_nonexistent_merchant_fails() {
    let (_env, client, _contract_id, _admin) = setup_test();

    client.get_merchant_webhook(&99u64);
}

#[test]
fn test_webhooks_are_per_merchant() {
    let (env, client, _contract_id, _admin) = setup_test();

    let merchant_1 = Address::generate(&env);
    let merchant_2 = Address::generate(&env);
    client.register_merchant(&merchant_1);
    client.register_merchant(&merchant_2);

    let webhook_1 = String::from_str(&env, "https://m1.example.com/hook");
    let webhook_2 = String::from_str(&env, "https://m2.example.com/hook");

    client.set_merchant_webhook(&merchant_1, &webhook_1);
    client.set_merchant_webhook(&merchant_2, &webhook_2);

    assert_eq!(client.get_merchant_webhook(&1u64), webhook_1);
    assert_eq!(client.get_merchant_webhook(&2u64), webhook_2);
}
