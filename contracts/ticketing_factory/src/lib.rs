#![no_std]

mod errors;
#[cfg(test)]
mod test;

use crate::errors::FactoryError;
use soroban_sdk::{
    contract, contractevent, contractimpl, contracttype, panic_with_error,
    Address, Bytes, BytesN, Env, Vec,
};

// ── Data Structures ────────────────────────────────────────────────────────────

/// Records a deployed ticketing contract instance associated with an organizer.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventRef {
    /// Sequential reference ID assigned by the factory.
    pub ref_id: u64,
    /// Address of the deployed ticketing contract.
    pub contract: Address,
    /// Organizer who requested the deployment.
    pub organizer: Address,
    /// Ledger timestamp at deployment time.
    pub deployed_at: u64,
}

// ── Storage Keys ───────────────────────────────────────────────────────────────

#[contracttype]
enum DataKey {
    Admin,
    TicketingWasmHash,
    EventRef(u64),
    EventRefCount,
}

// ── Events ─────────────────────────────────────────────────────────────────────

#[contractevent]
pub struct EventContractDeployedEvent {
    pub ref_id: u64,
    pub contract: Address,
    pub organizer: Address,
    pub timestamp: u64,
}

pub fn publish_event_contract_deployed(
    env: &Env,
    ref_id: u64,
    contract: Address,
    organizer: Address,
    timestamp: u64,
) {
    EventContractDeployedEvent {
        ref_id,
        contract,
        organizer,
        timestamp,
    }
    .publish(env);
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn require_admin(env: &Env, caller: &Address) {
    let admin: Address = env
        .storage()
        .persistent()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(env, FactoryError::NotInitialized));
    if &admin != caller {
        panic_with_error!(env, FactoryError::NotAuthorized);
    }
}

fn get_ref_count(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::EventRefCount)
        .unwrap_or(0)
}

// ── Contract ───────────────────────────────────────────────────────────────────

#[contract]
pub struct TicketingFactory;

#[contractimpl]
impl TicketingFactory {
    /// Initialize the factory with an admin address.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().persistent().has(&DataKey::Admin) {
            panic_with_error!(env, FactoryError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &admin);
    }

    /// Set the WASM hash of the ticketing contract to deploy.
    /// Only the admin may call this.
    pub fn set_ticketing_wasm_hash(env: Env, admin: Address, wasm_hash: BytesN<32>) {
        admin.require_auth();
        require_admin(&env, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::TicketingWasmHash, &wasm_hash);
    }

    /// Deploy a fresh ticketing contract for a large event.
    ///
    /// Uses the Soroban deployer with a random salt derived from on-chain
    /// randomness so each deployment lands at a unique address.  The deployed
    /// contract address and a sequential `ref_id` are stored on-chain for
    /// retrieval via `get_event_ref` / `get_all_event_refs`.
    ///
    /// Returns the `EventRef` describing the newly deployed contract.
    pub fn deploy_event_contract(env: Env, organizer: Address) -> EventRef {
        organizer.require_auth();

        let wasm_hash: BytesN<32> = env
            .storage()
            .persistent()
            .get(&DataKey::TicketingWasmHash)
            .unwrap_or_else(|| panic_with_error!(env, FactoryError::WasmHashNotSet));

        // Derive a unique salt from on-chain PRNG so each deployment is distinct.
        let random: BytesN<32> = env.prng().gen();
        let salt = env
            .crypto()
            .keccak256(&Bytes::from_slice(&env, &random.to_array()));

        let deployed = env
            .deployer()
            .with_current_contract(salt)
            .deploy_v2(wasm_hash, ());

        let now = env.ledger().timestamp();
        let ref_id = get_ref_count(&env) + 1;

        let event_ref = EventRef {
            ref_id,
            contract: deployed.clone(),
            organizer: organizer.clone(),
            deployed_at: now,
        };

        env.storage()
            .persistent()
            .set(&DataKey::EventRef(ref_id), &event_ref);
        env.storage()
            .persistent()
            .set(&DataKey::EventRefCount, &ref_id);

        publish_event_contract_deployed(&env, ref_id, deployed, organizer, now);

        event_ref
    }

    /// Retrieve the `EventRef` for a given reference ID.
    pub fn get_event_ref(env: Env, ref_id: u64) -> EventRef {
        env.storage()
            .persistent()
            .get(&DataKey::EventRef(ref_id))
            .unwrap_or_else(|| panic_with_error!(env, FactoryError::EventRefNotFound))
    }

    /// Total number of event contracts deployed through this factory.
    pub fn get_event_ref_count(env: Env) -> u64 {
        get_ref_count(&env)
    }

    /// Return all stored event references in insertion order.
    pub fn get_all_event_refs(env: Env) -> Vec<EventRef> {
        let count = get_ref_count(&env);
        let mut refs = Vec::new(&env);
        for i in 1..=count {
            if let Some(r) = env.storage().persistent().get(&DataKey::EventRef(i)) {
                refs.push_back(r);
            }
        }
        refs
    }
}
