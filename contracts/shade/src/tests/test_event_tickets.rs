#![cfg(test)]

use crate::shade::{Shade, ShadeClient};
use soroban_sdk::testutils::{Address as _};
use soroban_sdk::{Address, Env, String};

fn setup_test() -> (Env, ShadeClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, contract_id, admin)
}

fn create_test_token(env: &Env) -> Address {
    let token_admin = Address::generate(env);
    env.register_stellar_asset_contract_v2(token_admin)
        .address()
}

#[test]
fn test_event_creation_and_purchase() {
    let (env, client, _shade_id, admin) = setup_test();
    let token = create_test_token(&env);
    client.add_accepted_token(&admin, &token);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let event_id = client.create_event(
        &merchant,
        &String::from_str(&env, "Concert"),
        &100,
        &token,
        &2, // Capacity 2
    );

    let event = client.get_event(&event_id);
    assert_eq!(event.name, String::from_str(&env, "Concert"));
    assert_eq!(event.capacity, 2);
    assert_eq!(event.sold, 0);

    let buyer1 = Address::generate(&env);
    client.purchase_ticket(&event_id, &buyer1);
    
    let event = client.get_event(&event_id);
    assert_eq!(event.sold, 1);

    let buyer2 = Address::generate(&env);
    client.purchase_ticket(&event_id, &buyer2);
    
    let event = client.get_event(&event_id);
    assert_eq!(event.sold, 2);
}

#[test]
#[should_panic] // Capacity reached
fn test_event_sold_out() {
    let (env, client, _shade_id, admin) = setup_test();
    let token = create_test_token(&env);
    client.add_accepted_token(&admin, &token);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let event_id = client.create_event(
        &merchant,
        &String::from_str(&env, "Small Show"),
        &100,
        &token,
        &1, // Capacity 1
    );

    let buyer1 = Address::generate(&env);
    client.purchase_ticket(&event_id, &buyer1);

    let buyer2 = Address::generate(&env);
    client.purchase_ticket(&event_id, &buyer2); // Should panic
}
