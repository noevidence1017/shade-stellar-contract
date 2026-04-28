#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, BytesN, Env, String};

fn make_qr(env: &Env, seed: u8) -> BytesN<32> {
    let mut bytes = [0u8; 32];
    bytes[0] = seed;
    BytesN::from_array(env, &bytes)
}

// ── Full lifecycle ─────────────────────────────────────────────────────────────

/// Creates an event, issues a ticket, verifies it, checks it in, and confirms
/// all state transitions are reflected correctly.
#[test]
fn test_full_ticket_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(TicketingContract, ());
    let client = TicketingContractClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let holder = Address::generate(&env);
    let operator = Address::generate(&env);

    // 1. Create event
    let event_id = client.create_event(
        &organizer,
        &String::from_str(&env, "Shade Protocol Summit"),
        &String::from_str(&env, "Annual developer conference"),
        &1_800_000_u64,
        &1_900_000_u64,
        &Some(100_u64),
    );

    let event = client.get_event(&event_id);
    assert_eq!(event.organizer, organizer);
    assert_eq!(event.max_capacity, Some(100));

    // 2. Issue ticket
    let qr = make_qr(&env, 1);
    let ticket_id = client.issue_ticket(&organizer, &event_id, &holder, &qr);

    // 3. Verify before check-in
    let v = client.verify_ticket(&ticket_id, &qr);
    assert!(v.valid);
    assert!(!v.already_checked_in);
    assert_eq!(v.holder, holder);
    assert_eq!(v.event_id, event_id);

    // 4. Check in
    client.check_in(&operator, &ticket_id);

    let ticket = client.get_ticket(&ticket_id);
    assert!(ticket.checked_in);
    assert!(ticket.check_in_time.is_some());

    // 5. Verify after check-in
    let v2 = client.verify_ticket(&ticket_id, &qr);
    assert!(v2.valid);
    assert!(v2.already_checked_in);

    // 6. Check-in record
    let record = client.get_check_in_record(&ticket_id);
    assert!(record.is_some());
    let rec = record.unwrap();
    assert_eq!(rec.checked_in_by, operator);
    assert_eq!(rec.ticket_id, ticket_id);
}

// ── Data integrity across multiple events ──────────────────────────────────────

/// Two events in the same contract have separate, non-overlapping ticket pools.
#[test]
fn test_data_integrity_multiple_events() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(TicketingContract, ());
    let client = TicketingContractClient::new(&env, &contract_id);

    let organizer_a = Address::generate(&env);
    let organizer_b = Address::generate(&env);
    let holder_a1 = Address::generate(&env);
    let holder_a2 = Address::generate(&env);
    let holder_b1 = Address::generate(&env);

    let event_a = client.create_event(
        &organizer_a,
        &String::from_str(&env, "Event Alpha"),
        &String::from_str(&env, "First event"),
        &1000_u64,
        &2000_u64,
        &Some(50_u64),
    );

    let event_b = client.create_event(
        &organizer_b,
        &String::from_str(&env, "Event Beta"),
        &String::from_str(&env, "Second event"),
        &3000_u64,
        &4000_u64,
        &Some(30_u64),
    );

    // Issue tickets for Event A
    let qr_a1 = make_qr(&env, 10);
    let qr_a2 = make_qr(&env, 11);
    let ticket_a1 = client.issue_ticket(&organizer_a, &event_a, &holder_a1, &qr_a1);
    let ticket_a2 = client.issue_ticket(&organizer_a, &event_a, &holder_a2, &qr_a2);

    // Issue ticket for Event B
    let qr_b1 = make_qr(&env, 20);
    let ticket_b1 = client.issue_ticket(&organizer_b, &event_b, &holder_b1, &qr_b1);

    // Ticket counts are isolated per event
    assert_eq!(client.get_event_ticket_count(&event_a), 2);
    assert_eq!(client.get_event_ticket_count(&event_b), 1);

    // Each ticket's event_id is correct
    assert_eq!(client.get_ticket(&ticket_a1).event_id, event_a);
    assert_eq!(client.get_ticket(&ticket_a2).event_id, event_a);
    assert_eq!(client.get_ticket(&ticket_b1).event_id, event_b);

    // Check-in counts are independent
    let operator = Address::generate(&env);
    client.check_in(&operator, &ticket_a1);
    assert_eq!(client.get_event_checked_in_count(&event_a), 1);
    assert_eq!(client.get_event_checked_in_count(&event_b), 0);
}

/// Organizer A cannot issue a ticket for Organizer B's event.
#[test]
#[should_panic]
fn test_wrong_organizer_cannot_issue_for_other_event() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(TicketingContract, ());
    let client = TicketingContractClient::new(&env, &contract_id);

    let organizer_a = Address::generate(&env);
    let organizer_b = Address::generate(&env);
    let holder = Address::generate(&env);

    let event_b = client.create_event(
        &organizer_b,
        &String::from_str(&env, "Event B"),
        &String::from_str(&env, ""),
        &1000_u64,
        &2000_u64,
        &None::<u64>,
    );

    // organizer_a tries to issue for event_b — must panic
    client.issue_ticket(&organizer_a, &event_b, &holder, &make_qr(&env, 99));
}

// ── Factory-deployed contract isolation ────────────────────────────────────────

/// Simulates two contracts deployed by a factory.  Each contract has its own
/// counters and storage; the same ticket_id in different contracts refers to
/// completely different tickets.
#[test]
fn test_factory_deployed_contracts_are_isolated() {
    let env = Env::default();
    env.mock_all_auths();

    // Simulated factory deploys two separate ticketing contracts.
    let contract_1 = env.register(TicketingContract, ());
    let contract_2 = env.register(TicketingContract, ());

    let client_1 = TicketingContractClient::new(&env, &contract_1);
    let client_2 = TicketingContractClient::new(&env, &contract_2);

    let organizer_1 = Address::generate(&env);
    let organizer_2 = Address::generate(&env);
    let holder = Address::generate(&env);

    let event_1 = client_1.create_event(
        &organizer_1,
        &String::from_str(&env, "Event on Contract 1"),
        &String::from_str(&env, ""),
        &1000_u64,
        &2000_u64,
        &None::<u64>,
    );

    let qr_1 = make_qr(&env, 1);
    let ticket_1 = client_1.issue_ticket(&organizer_1, &event_1, &holder, &qr_1);

    let event_2 = client_2.create_event(
        &organizer_2,
        &String::from_str(&env, "Event on Contract 2"),
        &String::from_str(&env, ""),
        &1000_u64,
        &2000_u64,
        &None::<u64>,
    );

    let qr_2 = make_qr(&env, 2);
    let ticket_2 = client_2.issue_ticket(&organizer_2, &event_2, &holder, &qr_2);

    // Counters restart from 1 in each independent contract.
    assert_eq!(event_1, 1);
    assert_eq!(event_2, 1);
    assert_eq!(ticket_1, 1);
    assert_eq!(ticket_2, 1);

    // QR hash from contract 1 does not match the ticket in contract 2.
    let verification = client_2.verify_ticket(&ticket_2, &qr_1);
    assert!(!verification.valid);
}

// ── Transfer chain integrity ───────────────────────────────────────────────────

/// A ticket can be transferred multiple times before check-in; once checked
/// in it can no longer be transferred.
#[test]
fn test_transfer_chain_before_checkin() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(TicketingContract, ());
    let client = TicketingContractClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let holder1 = Address::generate(&env);
    let holder2 = Address::generate(&env);
    let holder3 = Address::generate(&env);

    let event_id = client.create_event(
        &organizer,
        &String::from_str(&env, "Transfer Test"),
        &String::from_str(&env, ""),
        &1000_u64,
        &2000_u64,
        &None::<u64>,
    );

    let qr = make_qr(&env, 55);
    let ticket_id = client.issue_ticket(&organizer, &event_id, &holder1, &qr);

    // holder1 → holder2
    client.transfer_ticket(&holder1, &ticket_id, &holder2);
    assert_eq!(client.get_ticket(&ticket_id).holder, holder2);

    // holder2 → holder3
    client.transfer_ticket(&holder2, &ticket_id, &holder3);
    assert_eq!(client.get_ticket(&ticket_id).holder, holder3);

    // Check in
    client.check_in(&organizer, &ticket_id);
    assert!(client.get_ticket(&ticket_id).checked_in);
}

/// Transfer after check-in must fail.
#[test]
#[should_panic]
fn test_transfer_after_checkin_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(TicketingContract, ());
    let client = TicketingContractClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let holder1 = Address::generate(&env);
    let holder2 = Address::generate(&env);

    let event_id = client.create_event(
        &organizer,
        &String::from_str(&env, "Transfer Test"),
        &String::from_str(&env, ""),
        &1000_u64,
        &2000_u64,
        &None::<u64>,
    );

    let ticket_id = client.issue_ticket(&organizer, &event_id, &holder1, &make_qr(&env, 1));
    client.check_in(&organizer, &ticket_id);

    // Must panic — checked-in tickets cannot be transferred
    client.transfer_ticket(&holder1, &ticket_id, &holder2);
}

// ── Capacity enforcement across tickets ────────────────────────────────────────

/// Exactly `max_capacity` tickets can be issued; the next one is rejected.
#[test]
fn test_capacity_enforced_until_full() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(TicketingContract, ());
    let client = TicketingContractClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let operator = Address::generate(&env);
    let holder = Address::generate(&env);

    let event_id = client.create_event(
        &organizer,
        &String::from_str(&env, "Sold Out Show"),
        &String::from_str(&env, ""),
        &1000_u64,
        &2000_u64,
        &Some(3_u64),
    );

    let t1 = client.issue_ticket(&organizer, &event_id, &holder, &make_qr(&env, 1));
    let t2 = client.issue_ticket(&organizer, &event_id, &holder, &make_qr(&env, 2));
    let t3 = client.issue_ticket(&organizer, &event_id, &holder, &make_qr(&env, 3));

    // All three tickets check in successfully.
    client.check_in(&operator, &t1);
    client.check_in(&operator, &t2);
    client.check_in(&operator, &t3);

    assert_eq!(client.get_event_checked_in_count(&event_id), 3);
    assert_eq!(client.get_event_ticket_count(&event_id), 3);
}

/// Issuing beyond max_capacity must fail.
#[test]
#[should_panic]
fn test_issue_beyond_capacity_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(TicketingContract, ());
    let client = TicketingContractClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let holder = Address::generate(&env);

    let event_id = client.create_event(
        &organizer,
        &String::from_str(&env, "Sold Out Show"),
        &String::from_str(&env, ""),
        &1000_u64,
        &2000_u64,
        &Some(2_u64),
    );

    client.issue_ticket(&organizer, &event_id, &holder, &make_qr(&env, 1));
    client.issue_ticket(&organizer, &event_id, &holder, &make_qr(&env, 2));
    // Must panic — at capacity
    client.issue_ticket(&organizer, &event_id, &holder, &make_qr(&env, 3));
}
