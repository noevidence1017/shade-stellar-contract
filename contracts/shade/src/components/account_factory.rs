use crate::errors::ContractError;
use crate::events;
use crate::types::DataKey;
use soroban_sdk::panic_with_error;
use soroban_sdk::{Address, Bytes, BytesN, Env, IntoVal};

pub fn deploy_account(env: &Env, merchant: Address, merchant_id: u64) -> Address {
    let manager = env.current_contract_address();
    let wasm_hash: BytesN<32> = env
        .storage()
        .persistent()
        .get(&DataKey::AccountWasmHash)
        .unwrap_or_else(|| {
            panic_with_error!(env, ContractError::WasmHashNotSet);
        });

    #[cfg(test)]
    if wasm_hash == BytesN::from_array(env, &[0; 32]) {
        use soroban_sdk::testutils::Address as _;
        // Return a mock address for testing without actual deployment
        let mock_address = Address::generate(env);

        events::publish_merchant_account_deployed_event(
            env,
            merchant,
            mock_address.clone(),
            env.ledger().timestamp(),
        );
        return mock_address;
    }

    // Generate a random salt for deployment.
    let random_bytes_n: BytesN<32> = env.prng().gen();
    let random_bytes = Bytes::from_slice(env, &random_bytes_n.to_array());
    let salt = env.crypto().keccak256(&random_bytes);

    let deployed_contract = env
        .deployer()
        .with_current_contract(salt)
        .deploy_v2(wasm_hash.clone(), ());

    // Initialize the deployed contract with the required arguments.
    env.invoke_contract::<()>(
        &deployed_contract,
        &soroban_sdk::Symbol::new(env, "initialize"),
        (merchant.clone(), manager, merchant_id).into_val(env),
    );

    events::publish_merchant_account_deployed_event(
        env,
        merchant,
        deployed_contract.clone(),
        env.ledger().timestamp(),
    );

    deployed_contract
}
