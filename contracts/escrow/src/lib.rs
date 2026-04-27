#![no_std]
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, token, Address, Env, String};

#[contract]
pub struct EscrowContract;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Buyer,
    Seller,
    Arbiter,
    Terms,
    Token,
    Amount,
    Status,
    Deadline,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowStatus {
    Pending,
    Completed,
    Disputed,
    Resolved,
    Expired,
}

#[contractevent]
pub struct EscrowInitializedEvent {
    pub buyer: Address,
    pub seller: Address,
    pub arbiter: Address,
    pub token: Address,
    pub amount: i128,
    pub deadline: u64,
}

#[contractevent]
pub struct EscrowReleaseApprovedEvent {
    pub buyer: Address,
    pub seller: Address,
    pub token: Address,
    pub amount: i128,
}

#[contractevent]
pub struct EscrowDisputeOpenedEvent {
    pub buyer: Address,
    pub token: Address,
    pub amount: i128,
}

#[contractevent]
pub struct EscrowDisputeResolvedEvent {
    pub arbiter: Address,
    pub recipient: Address,
    pub token: Address,
    pub amount: i128,
    pub released_to_buyer: bool,
}

#[contractevent]
pub struct EscrowArbitrationSplitEvent {
    pub arbiter: Address,
    pub buyer: Address,
    pub seller: Address,
    pub token: Address,
    pub buyer_amount: i128,
    pub seller_amount: i128,
}

/// Emitted when the escrow expires and funds are returned to the buyer.
#[contractevent]
pub struct EscrowExpiredEvent {
    pub buyer: Address,
    pub token: Address,
    pub amount: i128,
    pub deadline: u64,
}

#[contractimpl]
impl EscrowContract {
    /// Initialise the escrow with a future deadline timestamp.
    ///
    /// `deadline` is a Unix timestamp (seconds). It must be strictly greater
    /// than the current ledger timestamp at the time of initialisation.
    pub fn init(
        env: Env,
        buyer: Address,
        seller: Address,
        arbiter: Address,
        terms: String,
        token: Address,
        amount: i128,
        deadline: u64,
    ) {
        if env.storage().instance().has(&DataKey::Buyer) {
            panic!("escrow already initialized");
        }
        assert!(
            deadline > env.ledger().timestamp(),
            "deadline must be in the future"
        );
        env.storage().instance().set(&DataKey::Buyer, &buyer);
        env.storage().instance().set(&DataKey::Seller, &seller);
        env.storage().instance().set(&DataKey::Arbiter, &arbiter);
        env.storage().instance().set(&DataKey::Terms, &terms);
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::Amount, &amount);
        env.storage().instance().set(&DataKey::Deadline, &deadline);
        env.storage().instance().set(&DataKey::Status, &EscrowStatus::Pending);
        EscrowInitializedEvent { buyer, seller, arbiter, token, amount, deadline }.publish(&env);
    }

    pub fn buyer(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Buyer).unwrap()
    }

    pub fn seller(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Seller).unwrap()
    }

    pub fn arbiter(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Arbiter).unwrap()
    }

    pub fn terms(env: Env) -> String {
        env.storage().instance().get(&DataKey::Terms).unwrap()
    }

    pub fn token(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Token).unwrap()
    }

    pub fn amount(env: Env) -> i128 {
        env.storage().instance().get(&DataKey::Amount).unwrap()
    }

    pub fn status(env: Env) -> EscrowStatus {
        env.storage().instance().get(&DataKey::Status).unwrap()
    }

    /// Return the configured deadline timestamp.
    pub fn deadline(env: Env) -> u64 {
        env.storage().instance().get(&DataKey::Deadline).unwrap()
    }

    pub fn approve_release(env: Env) {
        let buyer = Self::buyer(env.clone());
        buyer.require_auth();
        if Self::status(env.clone()) != EscrowStatus::Pending {
            panic!("escrow is not pending");
        }
        assert!(
            env.ledger().timestamp() <= Self::deadline(env.clone()),
            "escrow has expired"
        );
        let seller = Self::seller(env.clone());
        let token = Self::token(env.clone());
        let amount = Self::amount(env.clone());
        token::TokenClient::new(&env, &token)
            .transfer(&env.current_contract_address(), &seller, &amount);
        env.storage().instance().set(&DataKey::Status, &EscrowStatus::Completed);
        EscrowReleaseApprovedEvent { buyer, seller, token, amount }.publish(&env);
    }

    pub fn open_dispute(env: Env) {
        let buyer = Self::buyer(env.clone());
        buyer.require_auth();
        if Self::status(env.clone()) != EscrowStatus::Pending {
            panic!("escrow is not pending");
        }
        assert!(
            env.ledger().timestamp() <= Self::deadline(env.clone()),
            "escrow has expired"
        );
        let token = Self::token(env.clone());
        let amount = Self::amount(env.clone());
        env.storage().instance().set(&DataKey::Status, &EscrowStatus::Disputed);
        EscrowDisputeOpenedEvent { buyer, token, amount }.publish(&env);
    }

    /// Expire the escrow and return funds to the buyer.
    ///
    /// Anyone can call this once the deadline has passed and the escrow is
    /// still `Pending`. Funds are transferred back to the buyer.
    pub fn expire(env: Env) {
        if Self::status(env.clone()) != EscrowStatus::Pending {
            panic!("escrow is not pending");
        }
        let deadline = Self::deadline(env.clone());
        assert!(
            env.ledger().timestamp() > deadline,
            "escrow deadline has not passed yet"
        );
        let buyer = Self::buyer(env.clone());
        let token = Self::token(env.clone());
        let amount = Self::amount(env.clone());
        token::TokenClient::new(&env, &token)
            .transfer(&env.current_contract_address(), &buyer, &amount);
        env.storage().instance().set(&DataKey::Status, &EscrowStatus::Expired);
        EscrowExpiredEvent { buyer, token, amount, deadline }.publish(&env);
    }

    pub fn resolve_dispute(env: Env, released_to_buyer: bool) {
        let arbiter = Self::arbiter(env.clone());
        arbiter.require_auth();
        if Self::status(env.clone()) != EscrowStatus::Disputed {
            panic!("escrow dispute is not open");
        }
        let buyer = Self::buyer(env.clone());
        let seller = Self::seller(env.clone());
        let token = Self::token(env.clone());
        let amount = Self::amount(env.clone());
        let recipient = if released_to_buyer { buyer } else { seller };
        token::TokenClient::new(&env, &token)
            .transfer(&env.current_contract_address(), &recipient, &amount);
        env.storage().instance().set(&DataKey::Status, &EscrowStatus::Resolved);
        EscrowDisputeResolvedEvent { arbiter, recipient, token, amount, released_to_buyer }.publish(&env);
    }

    pub fn resolve(env: Env, buyer_amount: i128, seller_amount: i128) {
        let arbiter = Self::arbiter(env.clone());
        arbiter.require_auth();
        if Self::status(env.clone()) != EscrowStatus::Disputed {
            panic!("escrow dispute is not open");
        }
        let total = Self::amount(env.clone());
        assert!(buyer_amount >= 0 && seller_amount >= 0, "amounts must be non-negative");
        assert!(buyer_amount + seller_amount == total, "buyer_amount + seller_amount must equal total escrowed amount");
        let buyer = Self::buyer(env.clone());
        let seller = Self::seller(env.clone());
        let token = Self::token(env.clone());
        let tc = token::TokenClient::new(&env, &token);
        if buyer_amount > 0 { tc.transfer(&env.current_contract_address(), &buyer, &buyer_amount); }
        if seller_amount > 0 { tc.transfer(&env.current_contract_address(), &seller, &seller_amount); }
        env.storage().instance().set(&DataKey::Status, &EscrowStatus::Resolved);
        EscrowArbitrationSplitEvent { arbiter, buyer, seller, token, buyer_amount, seller_amount }.publish(&env);
    }
}

mod test;