#![cfg(test)]

use crate::errors::ContractError;
use crate::shade::{Shade, ShadeClient};
use crate::types::Role;
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{Address, Env, Map, Symbol, TryIntoVal, Val};

fn setup() -> (Env, ShadeClient<'static>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let merchant = Address::generate(&env);
    let manager = Address::generate(&env);

    // Grant Manager role
    client.grant_role(&admin, &manager, &Role::Manager);

    // Register Merchant
    client.register_merchant(&merchant);

    // Deploy MerchantAccount mock (using real contract)
    let acct_id = env.register(account::account::MerchantAccount, ());
    let acct_client = account::account::MerchantAccountClient::new(&env, &acct_id);
    let merchant_id_val = 1_u64; // Since this is the first and only merchant
    acct_client.initialize(&merchant, &contract_id, &merchant_id_val);

    // Link Merchant Account in Shade -> simulate factory linking
    client.set_merchant_account(&merchant, &acct_id);

    (env, client, admin, manager, merchant)
}

fn assert_latest_account_restricted_event(
    env: &Env,
    contract_id: &Address,
    expected_merchant: &Address,
    expected_status: bool,
    expected_caller: &Address,
) {
    let events = env.events().all();
    assert!(!events.is_empty());

    let mut found = false;
    for i in (0..events.len()).rev() {
        let (event_contract_id_i, topics_i, data_i) = events.get(i).unwrap();
        if topics_i.len() == 1 {
            let event_name: Symbol = topics_i.get(0).unwrap().try_into_val(env).unwrap();
            if event_name == Symbol::new(env, "account_restricted_event") {
                assert_eq!(event_contract_id_i, contract_id.clone());
                let data_map: Map<Symbol, Val> = data_i.try_into_val(env).unwrap();
                let merchant_val = data_map.get(Symbol::new(env, "merchant")).unwrap();
                let status_val = data_map.get(Symbol::new(env, "status")).unwrap();
                let caller_val = data_map.get(Symbol::new(env, "caller")).unwrap();

                let merchant_in_event: Address = merchant_val.try_into_val(env).unwrap();
                let status_in_event: bool = status_val.try_into_val(env).unwrap();
                let caller_in_event: Address = caller_val.try_into_val(env).unwrap();

                assert_eq!(merchant_in_event, expected_merchant.clone());
                assert_eq!(status_in_event, expected_status);
                assert_eq!(caller_in_event, expected_caller.clone());
                found = true;
                break;
            }
        }
    }
    assert!(found, "account_restricted_event not found in events");
}

#[test]
fn test_admin_restrict_merchant_account_success() {
    let (env, client, admin, _manager, merchant) = setup();

    // Verify initial state
    let merchant_id = 1_u64;
    let account_address = client.get_merchant_account(&merchant_id);
    let acct_client = account::account::MerchantAccountClient::new(&env, &account_address);
    assert!(!acct_client.is_restricted_account());

    // Admin restricts the account
    client.restrict_merchant_account(&admin, &merchant, &true);

    // Verify Shade event
    assert_latest_account_restricted_event(&env, &client.address, &merchant, true, &admin);

    // Verify Account contract state changed
    assert!(acct_client.is_restricted_account());

    // Admin un-restricts the account
    client.restrict_merchant_account(&admin, &merchant, &false);
    assert_latest_account_restricted_event(&env, &client.address, &merchant, false, &admin);
    assert!(!acct_client.is_restricted_account());
}

#[test]
fn test_manager_restrict_merchant_account_success() {
    let (env, client, _admin, manager, merchant) = setup();

    let merchant_id = 1_u64;
    let account_address = client.get_merchant_account(&merchant_id);
    let acct_client = account::account::MerchantAccountClient::new(&env, &account_address);

    // Manager restricts the account
    client.restrict_merchant_account(&manager, &merchant, &true);

    // Verify Shape event
    assert_latest_account_restricted_event(&env, &client.address, &merchant, true, &manager);

    // Verify Account contract state
    assert!(acct_client.is_restricted_account());
}

#[test]
fn test_unauthorized_restriction_attempt() {
    let (env, client, _admin, _manager, merchant) = setup();

    let random_user = Address::generate(&env);

    // Attempt from random user
    let res = client.try_restrict_merchant_account(&random_user, &merchant, &true);
    assert_eq!(
        res,
        Err(Ok(soroban_sdk::Error::from_contract_error(
            ContractError::NotAuthorized as u32
        )))
    );

    // Attempt from the merchant themselves
    let res = client.try_restrict_merchant_account(&merchant, &merchant, &true);
    assert_eq!(
        res,
        Err(Ok(soroban_sdk::Error::from_contract_error(
            ContractError::NotAuthorized as u32
        )))
    );

    // Verify state did not change
    let merchant_id = 1_u64;
    let account_address = client.get_merchant_account(&merchant_id);
    let acct_client = account::account::MerchantAccountClient::new(&env, &account_address);
    assert!(!acct_client.is_restricted_account());
}

#[test]
fn test_invalid_merchant_restriction() {
    let (env, client, admin, _manager, _merchant) = setup();
    let invalid_merchant = Address::generate(&env);

    let res = client.try_restrict_merchant_account(&admin, &invalid_merchant, &true);
    assert_eq!(
        res,
        Err(Ok(soroban_sdk::Error::from_contract_error(
            ContractError::MerchantNotFound as u32
        )))
    );
}
