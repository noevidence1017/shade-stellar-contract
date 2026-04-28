#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};

// ── Helpers ────────────────────────────────────────────────────────────────────

fn setup(env: &Env) -> (Address, TicketingFactoryClient) {
    let contract_id = env.register(TicketingFactory, ());
    let client = TicketingFactoryClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    (admin, client)
}

// ── Initialization ─────────────────────────────────────────────────────────────

#[test]
fn test_initialize_sets_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, _client) = setup(&env);
    // No panic means initialization succeeded.
}

#[test]
#[should_panic]
fn test_double_initialize_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client) = setup(&env);
    // Second initialization should panic with AlreadyInitialized.
    client.initialize(&admin);
}

// ── WASM hash management ───────────────────────────────────────────────────────

#[test]
fn test_set_ticketing_wasm_hash_by_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client) = setup(&env);

    let wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
    client.set_ticketing_wasm_hash(&admin, &wasm_hash);
    // No panic means the hash was accepted.
}

#[test]
#[should_panic]
fn test_set_ticketing_wasm_hash_non_admin_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, client) = setup(&env);

    let impostor = Address::generate(&env);
    let wasm_hash = BytesN::from_array(&env, &[2u8; 32]);
    client.set_ticketing_wasm_hash(&impostor, &wasm_hash);
}

// ── Reference storage (simulated deployment) ───────────────────────────────────

/// Simulates what `deploy_event_contract` would do without an actual WASM
/// binary: registers a ticketing contract directly and stores its reference
/// via the factory's internal helpers.  This mirrors the pattern used in
/// the account-factory tests.
fn register_event_ref(
    env: &Env,
    factory_id: &Address,
    organizer: Address,
    contract: Address,
) -> EventRef {
    env.as_contract(factory_id, || {
        let ref_id = get_ref_count(env) + 1;
        let now = env.ledger().timestamp();
        let event_ref = EventRef {
            ref_id,
            contract: contract.clone(),
            organizer: organizer.clone(),
            deployed_at: now,
        };
        env.storage()
            .persistent()
            .set(&DataKey::EventRef(ref_id), &event_ref);
        env.storage()
            .persistent()
            .set(&DataKey::EventRefCount, &ref_id);
        publish_event_contract_deployed(env, ref_id, contract, organizer, now);
        event_ref
    })
}

#[test]
fn test_get_event_ref_after_registration() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, client) = setup(&env);
    let factory_id = env.register(TicketingFactory, ());

    let organizer = Address::generate(&env);
    let ticketing_contract = Address::generate(&env);

    let stored = register_event_ref(&env, &factory_id, organizer.clone(), ticketing_contract.clone());
    let factory_client = TicketingFactoryClient::new(&env, &factory_id);

    let fetched = factory_client.get_event_ref(&stored.ref_id);
    assert_eq!(fetched.ref_id, 1);
    assert_eq!(fetched.contract, ticketing_contract);
    assert_eq!(fetched.organizer, organizer);
}

#[test]
fn test_get_event_ref_count() {
    let env = Env::default();
    env.mock_all_auths();
    let factory_id = env.register(TicketingFactory, ());
    let factory_client = TicketingFactoryClient::new(&env, &factory_id);
    let admin = Address::generate(&env);
    factory_client.initialize(&admin);

    assert_eq!(factory_client.get_event_ref_count(), 0);

    let organizer = Address::generate(&env);
    register_event_ref(&env, &factory_id, organizer.clone(), Address::generate(&env));
    assert_eq!(factory_client.get_event_ref_count(), 1);

    register_event_ref(&env, &factory_id, organizer.clone(), Address::generate(&env));
    assert_eq!(factory_client.get_event_ref_count(), 2);
}

#[test]
fn test_get_all_event_refs_returns_all() {
    let env = Env::default();
    env.mock_all_auths();
    let factory_id = env.register(TicketingFactory, ());
    let factory_client = TicketingFactoryClient::new(&env, &factory_id);
    let admin = Address::generate(&env);
    factory_client.initialize(&admin);

    let organizer = Address::generate(&env);
    let c1 = Address::generate(&env);
    let c2 = Address::generate(&env);
    let c3 = Address::generate(&env);

    register_event_ref(&env, &factory_id, organizer.clone(), c1.clone());
    register_event_ref(&env, &factory_id, organizer.clone(), c2.clone());
    register_event_ref(&env, &factory_id, organizer.clone(), c3.clone());

    let all = factory_client.get_all_event_refs();
    assert_eq!(all.len(), 3);

    let mut found_c1 = false;
    let mut found_c2 = false;
    let mut found_c3 = false;
    for r in all.iter() {
        if r.contract == c1 { found_c1 = true; }
        if r.contract == c2 { found_c2 = true; }
        if r.contract == c3 { found_c3 = true; }
    }
    assert!(found_c1);
    assert!(found_c2);
    assert!(found_c3);
}

#[test]
#[should_panic]
fn test_get_nonexistent_event_ref_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, client) = setup(&env);
    client.get_event_ref(&999_u64);
}

// ── Factory + ticketing integration simulation ─────────────────────────────────

/// Simulates a complete factory → ticketing lifecycle:
///   factory registers deployment → ticketing contract creates event
///   → tickets issued → check-in verified.
#[test]
fn test_factory_and_ticketing_contract_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    // Set up factory
    let factory_id = env.register(TicketingFactory, ());
    let factory_client = TicketingFactoryClient::new(&env, &factory_id);
    let admin = Address::generate(&env);
    factory_client.initialize(&admin);

    let organizer = Address::generate(&env);

    // Simulate factory deploying a ticketing contract
    let ticketing_id = env.register(ticketing::TicketingContract, ());
    register_event_ref(&env, &factory_id, organizer.clone(), ticketing_id.clone());

    // Confirm factory holds the reference
    let event_ref = factory_client.get_event_ref(&1_u64);
    assert_eq!(event_ref.contract, ticketing_id);
    assert_eq!(event_ref.organizer, organizer);

    // Use the deployed ticketing contract
    let ticketing_client = ticketing::TicketingContractClient::new(&env, &ticketing_id);

    let event_id = ticketing_client.create_event(
        &organizer,
        &soroban_sdk::String::from_str(&env, "Shade Protocol Summit"),
        &soroban_sdk::String::from_str(&env, "Annual developer conference"),
        &1_800_000_u64,
        &1_900_000_u64,
        &Some(500_u64),
    );
    assert_eq!(event_id, 1);

    let holder = Address::generate(&env);
    let mut qr_bytes = [0u8; 32];
    qr_bytes[0] = 42;
    let qr_hash = soroban_sdk::BytesN::from_array(&env, &qr_bytes);
    let ticket_id = ticketing_client.issue_ticket(&organizer, &event_id, &holder, &qr_hash);

    let operator = Address::generate(&env);
    ticketing_client.check_in(&operator, &ticket_id);

    let ticket = ticketing_client.get_ticket(&ticket_id);
    assert!(ticket.checked_in);
    assert_eq!(ticket.holder, holder);
}

/// Two separately deployed ticketing contracts have fully isolated state.
#[test]
fn test_two_deployed_contracts_have_isolated_state() {
    let env = Env::default();
    env.mock_all_auths();

    let factory_id = env.register(TicketingFactory, ());
    let factory_client = TicketingFactoryClient::new(&env, &factory_id);
    let admin = Address::generate(&env);
    factory_client.initialize(&admin);

    let organizer = Address::generate(&env);

    let ticketing_a = env.register(ticketing::TicketingContract, ());
    let ticketing_b = env.register(ticketing::TicketingContract, ());

    register_event_ref(&env, &factory_id, organizer.clone(), ticketing_a.clone());
    register_event_ref(&env, &factory_id, organizer.clone(), ticketing_b.clone());

    assert_eq!(factory_client.get_event_ref_count(), 2);

    let client_a = ticketing::TicketingContractClient::new(&env, &ticketing_a);
    let client_b = ticketing::TicketingContractClient::new(&env, &ticketing_b);

    // Each contract starts with an independent event counter.
    let event_a = client_a.create_event(
        &organizer,
        &soroban_sdk::String::from_str(&env, "Event A"),
        &soroban_sdk::String::from_str(&env, ""),
        &1000_u64,
        &2000_u64,
        &None::<u64>,
    );

    let event_b = client_b.create_event(
        &organizer,
        &soroban_sdk::String::from_str(&env, "Event B"),
        &soroban_sdk::String::from_str(&env, ""),
        &1000_u64,
        &2000_u64,
        &None::<u64>,
    );

    // Both start at event_id = 1, isolated per contract.
    assert_eq!(event_a, 1);
    assert_eq!(event_b, 1);

    // Events are not visible cross-contract.
    let evt_a = client_a.get_event(&event_a);
    assert_eq!(evt_a.name, soroban_sdk::String::from_str(&env, "Event A"));

    let evt_b = client_b.get_event(&event_b);
    assert_eq!(evt_b.name, soroban_sdk::String::from_str(&env, "Event B"));
}
