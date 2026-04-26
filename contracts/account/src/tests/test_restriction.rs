#![cfg(test)]

use crate::account::{MerchantAccount, MerchantAccountClient};
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{Address, Env, Map, Symbol, TryIntoVal, Val};

fn setup_test() -> (
    Env,
    MerchantAccountClient<'static>,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(MerchantAccount, ());
    let client = MerchantAccountClient::new(&env, &contract_id);
    let merchant = Address::generate(&env);
    let manager = Address::generate(&env);
    client.initialize(&merchant, &manager, &1);
    (env, client, contract_id, merchant, manager)
}

fn find_account_restricted_event(env: &Env, contract_id: &Address) -> Option<(bool, u64)> {
    let events = env.events().all();

    for i in (0..events.len()).rev() {
        let (event_contract_id, topics, data) = events.get(i).unwrap();
        if &event_contract_id != contract_id {
            continue;
        }

        if topics.len() != 1 {
            continue;
        }

        let event_name: Symbol = topics.get(0).unwrap().try_into_val(env).unwrap();
        if event_name == Symbol::new(env, "account_restricted") {
            let data_map: Map<Symbol, Val> = data.try_into_val(env).unwrap();
            let status_val = data_map.get(Symbol::new(env, "status")).unwrap();
            let timestamp_val = data_map.get(Symbol::new(env, "timestamp")).unwrap();

            let status: bool = status_val.try_into_val(env).unwrap();
            let timestamp: u64 = timestamp_val.try_into_val(env).unwrap();

            return Some((status, timestamp));
        }
    }

    None
}

#[test]
fn test_default_state_is_not_restricted() {
    let (_env, client, _contract_id, _merchant, _manager) = setup_test();

    assert!(!client.is_restricted_account());
}

#[test]
fn test_manager_can_restrict_account() {
    let (_env, client, _contract_id, _merchant, _manager) = setup_test();

    client.restrict_account(&true);
    assert!(client.is_restricted_account());
}

#[test]
fn test_manager_can_unrestrict_account() {
    let (_env, client, _contract_id, _merchant, _manager) = setup_test();

    client.restrict_account(&true);
    assert!(client.is_restricted_account());

    client.restrict_account(&false);
    assert!(!client.is_restricted_account());
}

#[test]
fn test_restriction_status_persists() {
    let (_env, client, _contract_id, _merchant, _manager) = setup_test();

    client.restrict_account(&true);
    assert!(client.is_restricted_account());
    assert!(client.is_restricted_account());

    client.restrict_account(&false);
    assert!(!client.is_restricted_account());
    assert!(!client.is_restricted_account());
}

#[test]
#[should_panic(expected = "HostError: Error(Auth, InvalidAction)")]
fn test_merchant_cannot_restrict_account() {
    let (env, client, _contract_id, _merchant, _manager) = setup_test();

    env.set_auths(&[]);
    client.restrict_account(&true);
}

#[test]
#[should_panic(expected = "HostError: Error(Auth, InvalidAction)")]
fn test_guest_cannot_restrict_account() {
    let (env, client, _contract_id, _merchant, _manager) = setup_test();

    env.set_auths(&[]);
    client.restrict_account(&true);
}

#[test]
fn test_multiple_restriction_toggles() {
    let (_env, client, _contract_id, _merchant, _manager) = setup_test();

    assert!(!client.is_restricted_account());

    client.restrict_account(&true);
    assert!(client.is_restricted_account());

    client.restrict_account(&false);
    assert!(!client.is_restricted_account());

    client.restrict_account(&true);
    assert!(client.is_restricted_account());

    client.restrict_account(&true);
    assert!(client.is_restricted_account());

    client.restrict_account(&false);
    assert!(!client.is_restricted_account());
}

#[test]
fn test_restriction_emits_event_on_change() {
    let (env, client, contract_id, _merchant, _manager) = setup_test();

    client.restrict_account(&true);
    let event1 = find_account_restricted_event(&env, &contract_id);
    assert!(event1.is_some());
    let (status1, _) = event1.unwrap();
    assert!(status1);

    client.restrict_account(&false);
    let event2 = find_account_restricted_event(&env, &contract_id);
    assert!(event2.is_some());
    let (status2, _) = event2.unwrap();
    assert!(!status2);
}
