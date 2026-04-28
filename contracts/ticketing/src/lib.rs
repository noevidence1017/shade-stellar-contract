#![no_std]

mod errors;
#[cfg(test)]
mod test_integration;

use crate::errors::TicketingError;
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, panic_with_error, Address, BytesN, Env, String, Vec};

const HASH_LENGTH: usize = 32;

// ── Data Structures ────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    pub event_id: u64,
    pub organizer: Address,
    pub name: String,
    pub description: String,
    pub start_time: u64,
    pub end_time: u64,
    pub max_capacity: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ticket {
    pub ticket_id: u64,
    pub event_id: u64,
    pub holder: Address,
    pub qr_hash: BytesN<32>,
    pub checked_in: bool,
    pub check_in_time: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckInRecord {
    pub ticket_id: u64,
    pub checked_in_by: Address,
    pub check_in_time: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TicketVerification {
    pub ticket_id: u64,
    pub event_id: u64,
    pub holder: Address,
    pub valid: bool,
    pub already_checked_in: bool,
}

// ── Storage Keys ───────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Event(u64),
    Ticket(u64),
    EventCount,
    TicketCount,
    EventTickets(u64),         // Vec<u64> - ticket IDs for an event
    CheckInRecord(u64),        // CheckInRecord by ticket_id
}

// ── Events ─────────────────────────────────────────────────────────────────────

#[contractevent]
pub struct EventCreatedEvent {
    pub event_id: u64,
    pub organizer: Address,
    pub name: String,
    pub timestamp: u64,
}

pub fn publish_event_created_event(
    env: &Env,
    event_id: u64,
    organizer: Address,
    name: String,
    timestamp: u64,
) {
    EventCreatedEvent {
        event_id,
        organizer,
        name,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct TicketIssuedEvent {
    pub ticket_id: u64,
    pub event_id: u64,
    pub holder: Address,
    pub qr_hash: BytesN<32>,
    pub timestamp: u64,
}

pub fn publish_ticket_issued_event(
    env: &Env,
    ticket_id: u64,
    event_id: u64,
    holder: Address,
    qr_hash: BytesN<32>,
    timestamp: u64,
) {
    TicketIssuedEvent {
        ticket_id,
        event_id,
        holder,
        qr_hash,
        timestamp,
    }
    .publish(env);
}

#[contractevent]
pub struct TicketCheckedInEvent {
    pub ticket_id: u64,
    pub event_id: u64,
    pub holder: Address,
    pub checked_in_by: Address,
    pub check_in_time: u64,
}

pub fn publish_ticket_checked_in_event(
    env: &Env,
    ticket_id: u64,
    event_id: u64,
    holder: Address,
    checked_in_by: Address,
    check_in_time: u64,
) {
    TicketCheckedInEvent {
        ticket_id,
        event_id,
        holder,
        checked_in_by,
        check_in_time,
    }
    .publish(env);
}

#[contractevent]
pub struct TicketTransferedEvent {
    pub ticket_id: u64,
    pub event_id: u64,
    pub old_holder: Address,
    pub new_holder: Address,
    pub timestamp: u64,
}

pub fn publish_ticket_transferred_event(
    env: &Env,
    ticket_id: u64,
    event_id: u64,
    old_holder: Address,
    new_holder: Address,
    timestamp: u64,
) {
    TicketTransferedEvent {
        ticket_id,
        event_id,
        old_holder,
        new_holder,
        timestamp,
    }
    .publish(env);
}

// ── Helper Functions ───────────────────────────────────────────────────────────

fn get_event_count(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::EventCount)
        .unwrap_or(0)
}

fn get_ticket_count(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::TicketCount)
        .unwrap_or(0)
}

fn increment_event_count(env: &Env, count: u64) {
    env.storage()
        .persistent()
        .set(&DataKey::EventCount, &count);
}

fn increment_ticket_count(env: &Env, count: u64) {
    env.storage()
        .persistent()
        .set(&DataKey::TicketCount, &count);
}

fn add_ticket_to_event(env: &Env, event_id: u64, ticket_id: u64) {
    let key = DataKey::EventTickets(event_id);
    let mut tickets: Vec<u64> = env.storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    tickets.push_back(ticket_id);
    env.storage()
        .persistent()
        .set(&key, &tickets);
}

fn is_event_organizer(env: &Env, event_id: u64, organizer: &Address) -> bool {
    let event: Event = env.storage()
        .persistent()
        .get(&DataKey::Event(event_id))
        .unwrap_or_else(|| panic_with_error!(env, TicketingError::EventNotFound));
    &event.organizer == organizer
}

// ── Contract ───────────────────────────────────────────────────────────────────

#[contract]
pub struct TicketingContract;

#[contractimpl]
impl TicketingContract {
    /// Creates a new event. Only the organizer can later issue tickets for this event.
    pub fn create_event(
        env: Env,
        organizer: Address,
        name: String,
        description: String,
        start_time: u64,
        end_time: u64,
        max_capacity: Option<u64>,
    ) -> u64 {
        organizer.require_auth();

        if end_time < start_time {
            panic_with_error!(env, TicketingError::InvalidTimeRange);
        }

        let event_count = get_event_count(&env);
        let new_event_id = event_count + 1;

        let event = Event {
            event_id: new_event_id,
            organizer: organizer.clone(),
            name,
            description,
            start_time,
            end_time,
            max_capacity,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Event(new_event_id), &event);

        increment_event_count(&env, new_event_id);

        publish_event_created_event(
            &env,
            new_event_id,
            organizer,
            event.name,
            env.ledger().timestamp(),
        );

        new_event_id
    }

    /// Get event details by ID.
    pub fn get_event(env: Env, event_id: u64) -> Event {
        env.storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .unwrap_or_else(|| panic_with_error!(env, TicketingError::EventNotFound))
    }

    /// Issue a ticket for an event.
    /// The qr_hash must be a secure unique hash (SHA256) generated off-chain.
    pub fn issue_ticket(
        env: Env,
        organizer: Address,
        event_id: u64,
        holder: Address,
        qr_hash: BytesN<32>,
    ) -> u64 {
        organizer.require_auth();

        // Verify event exists and organizer owns it
        let event: Event = env.storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .unwrap_or_else(|| panic_with_error!(env, TicketingError::EventNotFound));

        if event.organizer != organizer {
            panic_with_error!(env, TicketingError::NotAuthorized);
        }

        // Check capacity if set
        if let Some(max_cap) = event.max_capacity {
            let tickets: Vec<u64> = env.storage()
                .persistent()
                .get(&DataKey::EventTickets(event_id))
                .unwrap_or_else(|| Vec::new(&env));
            if tickets.len() as u64 >= max_cap {
                panic_with_error!(env, TicketingError::EventAtCapacity);
            }
        }

        // Ensure QR hash uniqueness (no duplicate hashes across all tickets)
        let ticket_count = get_ticket_count(&env);
        for i in 1..=ticket_count {
            if let Some(ticket) = env.storage()
                .persistent()
                .get::<_, Ticket>(&DataKey::Ticket(i))
            {
                if ticket.qr_hash == qr_hash {
                    panic_with_error!(env, TicketingError::DuplicateQRHash);
                }
            }
        }

        let ticket_count = get_ticket_count(&env);
        let new_ticket_id = ticket_count + 1;

        let ticket = Ticket {
            ticket_id: new_ticket_id,
            event_id,
            holder: holder.clone(),
            qr_hash: qr_hash.clone(),
            checked_in: false,
            check_in_time: None,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Ticket(new_ticket_id), &ticket);

        add_ticket_to_event(&env, event_id, new_ticket_id);
        increment_ticket_count(&env, new_ticket_id);

        publish_ticket_issued_event(
            &env,
            new_ticket_id,
            event_id,
            holder,
            qr_hash,
            env.ledger().timestamp(),
        );

        new_ticket_id
    }

    /// Get ticket details by ID.
    pub fn get_ticket(env: Env, ticket_id: u64) -> Ticket {
        env.storage()
            .persistent()
            .get(&DataKey::Ticket(ticket_id))
            .unwrap_or_else(|| panic_with_error!(env, TicketingError::TicketNotFound))
    }

    /// Get all tickets for an event.
    pub fn get_event_tickets(env: Env, event_id: u64) -> Vec<Ticket> {
        let ticket_ids: Vec<u64> = env.storage()
            .persistent()
            .get(&DataKey::EventTickets(event_id))
            .unwrap_or_else(|| Vec::new(&env));

        let mut tickets = Vec::new(&env);
        for ticket_id in ticket_ids.iter() {
            let ticket: Ticket = env.storage()
                .persistent()
                .get(&DataKey::Ticket(ticket_id))
                .unwrap();
            tickets.push_back(ticket);
        }
        tickets
    }

    /// Verify a ticket by comparing the provided QR hash with stored hash.
    /// Returns ticket verification status without marking as checked in.
    pub fn verify_ticket(env: Env, ticket_id: u64, qr_hash: BytesN<32>) -> TicketVerification {
        let ticket: Ticket = env.storage()
            .persistent()
            .get(&DataKey::Ticket(ticket_id))
            .unwrap_or_else(|| panic_with_error!(env, TicketingError::TicketNotFound));

        let event_id = ticket.event_id;

        let valid = ticket.qr_hash == qr_hash;
        let already_checked_in = ticket.checked_in;

        TicketVerification {
            ticket_id,
            event_id,
            holder: ticket.holder,
            valid,
            already_checked_in,
        }
    }

    /// Check in a ticket. This is a state transition: pending → checked_in.
    /// Cannot be undone. Duplicate check-ins are rejected.
    pub fn check_in(env: Env, operator: Address, ticket_id: u64) {
        operator.require_auth();

        // Get the ticket
        let mut ticket: Ticket = env.storage()
            .persistent()
            .get(&DataKey::Ticket(ticket_id))
            .unwrap_or_else(|| panic_with_error!(env, TicketingError::TicketNotFound));

        // Prevent double check-in (idempotency)
        if ticket.checked_in {
            panic_with_error!(env, TicketingError::AlreadyCheckedIn);
        }

        let check_in_time = env.ledger().timestamp();

        ticket.checked_in = true;
        ticket.check_in_time = Some(check_in_time);

        // Save updated ticket
        env.storage()
            .persistent()
            .set(&DataKey::Ticket(ticket_id), &ticket);

        // Record check-in metadata (who scanned, when)
        let check_in_record = CheckInRecord {
            ticket_id,
            checked_in_by: operator.clone(),
            check_in_time,
        };
        env.storage()
            .persistent()
            .set(&DataKey::CheckInRecord(ticket_id), &check_in_record);

        // Emit event
        publish_ticket_checked_in_event(
            &env,
            ticket_id,
            ticket.event_id,
            ticket.holder,
            operator,
            check_in_time,
        );
    }

    /// Transfer a ticket from current holder to a new holder.
    /// Cannot transfer a checked-in ticket.
    pub fn transfer_ticket(
        env: Env,
        current_holder: Address,
        ticket_id: u64,
        new_holder: Address,
    ) {
        current_holder.require_auth();

        let mut ticket: Ticket = env.storage()
            .persistent()
            .get(&DataKey::Ticket(ticket_id))
            .unwrap_or_else(|| panic_with_error!(env, TicketingError::TicketNotFound));

        if ticket.holder != current_holder {
            panic_with_error!(env, TicketingError::NotAuthorized);
        }

        if ticket.checked_in {
            panic_with_error!(env, TicketingError::TicketAlreadyCheckedIn);
        }

        let old_holder = ticket.holder.clone();
        ticket.holder = new_holder.clone();

        env.storage()
            .persistent()
            .set(&DataKey::Ticket(ticket_id), &ticket);

        publish_ticket_transferred_event(
            &env,
            ticket_id,
            ticket.event_id,
            old_holder,
            new_holder,
            env.ledger().timestamp(),
        );
    }

    /// Get the check-in record for a ticket.
    pub fn get_check_in_record(env: Env, ticket_id: u64) -> Option<CheckInRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::CheckInRecord(ticket_id))
    }

    /// Get total number of tickets issued for an event.
    pub fn get_event_ticket_count(env: Env, event_id: u64) -> u64 {
        let ticket_ids: Vec<u64> = env.storage()
            .persistent()
            .get(&DataKey::EventTickets(event_id))
            .unwrap_or_else(|| Vec::new(&env));
        ticket_ids.len() as u64
    }

    /// Get total number of checked-in tickets for an event.
    pub fn get_event_checked_in_count(env: Env, event_id: u64) -> u64 {
        let ticket_ids: Vec<u64> = env.storage()
            .persistent()
            .get(&DataKey::EventTickets(event_id))
            .unwrap_or_else(|| Vec::new(&env));
        let mut count = 0;
        for ticket_id in ticket_ids.iter() {
            let ticket: Ticket = env.storage()
                .persistent()
                .get(&DataKey::Ticket(ticket_id))
                .unwrap();
            if ticket.checked_in {
                count += 1;
            }
        }
        count
    }
}
