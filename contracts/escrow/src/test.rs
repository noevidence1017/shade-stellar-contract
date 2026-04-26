#![cfg(test)]

use super::*;
use soroban_sdk::token;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn init_stores_roles_terms_token_and_amount() {
    let env = Env::default();
    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let arbiter = Address::generate(&env);
    let terms = String::from_str(&env, "Deliver goods by 2026-05-01");
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(token_admin).address();

    client.init(&buyer, &seller, &arbiter, &terms, &token, &7500i128);

    assert_eq!(client.buyer(), buyer);
    assert_eq!(client.seller(), seller);
    assert_eq!(client.arbiter(), arbiter);
    assert_eq!(client.terms(), terms);
    assert_eq!(client.token(), token);
    assert_eq!(client.amount(), 7500);
    assert_eq!(client.status(), EscrowStatus::Pending);
}

#[test]
fn buyer_can_approve_release() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let arbiter = Address::generate(&env);
    let terms = String::from_str(&env, "Deliver goods by 2026-05-01");
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(token_admin).address();

    client.init(&buyer, &seller, &arbiter, &terms, &token, &5000i128);

    let token_client = token::StellarAssetClient::new(&env, &token);
    token_client.mint(&contract_id, &5000);

    client.approve_release();

    assert_eq!(client.status(), EscrowStatus::Completed);
    assert_eq!(token_client.balance(&seller), 5000);
}

#[test]
fn buyer_can_open_dispute_and_arbiter_resolve() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let arbiter = Address::generate(&env);
    let terms = String::from_str(&env, "Deliver goods by 2026-05-01");
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(token_admin).address();

    client.init(&buyer, &seller, &arbiter, &terms, &token, &9000i128);

    let token_client = token::StellarAssetClient::new(&env, &token);
    token_client.mint(&contract_id, &9000);

    client.open_dispute();
    assert_eq!(client.status(), EscrowStatus::Disputed);

    client.resolve_dispute(&true);

    assert_eq!(client.status(), EscrowStatus::Resolved);
    assert_eq!(token_client.balance(&buyer), 9000);
}
