#![cfg(test)]
extern crate alloc;

use crate::shade::{Shade, ShadeClient};
use crate::types::Role;
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{Address, Bytes, BytesN, Env, String};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup_test() -> (Env, ShadeClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, contract_id, admin)
}

struct TestKeypair {
    signing_key: SigningKey,
    public_key_bytes: [u8; 32],
}

fn generate_keypair() -> TestKeypair {
    let signing_key = SigningKey::generate(&mut OsRng);
    let public_key_bytes: [u8; 32] = signing_key.verifying_key().to_bytes();
    TestKeypair {
        signing_key,
        public_key_bytes,
    }
}

/// Build the same message that `signature_util::build_message` constructs.
///
/// Format: [contract_address, merchant_address, nonce, amount, token_address, description_bytes]
fn build_test_message(
    env: &Env,
    contract_id: &Address,
    merchant: &Address,
    description: &String,
    amount: i128,
    token: &Address,
    nonce: &BytesN<32>,
) -> alloc::vec::Vec<u8> {
    let mut msg = Bytes::new(env);
    msg.append(&contract_id.clone().to_xdr(env));
    msg.append(&Bytes::from_array(env, b"|"));
    msg.append(&merchant.clone().to_xdr(env));
    msg.append(&Bytes::from_array(env, b"|"));
    msg.append(nonce.as_ref());
    msg.append(&Bytes::from_array(env, b"|"));
    msg.append(&Bytes::from_slice(env, &amount.to_be_bytes()));
    msg.append(&Bytes::from_array(env, b"|"));
    msg.append(&token.clone().to_xdr(env));
    msg.append(&Bytes::from_array(env, b"|"));
    msg.append(&description.clone().to_xdr(env));

    let mut result = alloc::vec![0u8; msg.len() as usize];
    for i in 0..msg.len() {
        result[i as usize] = msg.get(i).unwrap();
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn sign_invoice(
    env: &Env,
    contract_id: &Address,
    keypair: &TestKeypair,
    merchant: &Address,
    description: &String,
    amount: i128,
    token: &Address,
    nonce: &BytesN<32>,
) -> BytesN<64> {
    let message = build_test_message(
        env,
        contract_id,
        merchant,
        description,
        amount,
        token,
        nonce,
    );
    let sig = keypair.signing_key.sign(&message);
    BytesN::from_array(env, &sig.to_bytes())
}

fn create_nonce(env: &Env, seed: u8) -> BytesN<32> {
    let mut nonce: [u8; 32] = [0; 32];
    for (i, item) in nonce.iter_mut().enumerate() {
        *item = (i as u8).wrapping_add(seed);
    }
    BytesN::from_array(env, &nonce)
}

// ---------------------------------------------------------------------------
// Test cases
// ---------------------------------------------------------------------------

/// Test 1: Valid signature – a correctly signed invoice is accepted,
/// and the created invoice has the expected parameters.
#[test]
fn test_valid_signature() {
    let (env, client, contract_id, admin) = setup_test();

    // Setup: merchant + keypair
    let manager = Address::generate(&env);
    client.grant_role(&admin, &manager, &Role::Manager);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let keypair = generate_keypair();
    let pub_key = BytesN::from_array(&env, &keypair.public_key_bytes);
    client.set_merchant_key(&merchant, &pub_key);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Valid Signature Test");
    let amount: i128 = 5000;
    let nonce = create_nonce(&env, 1);

    let signature = sign_invoice(
        &env,
        &contract_id,
        &keypair,
        &merchant,
        &description,
        amount,
        &token,
        &nonce,
    );

    let invoice_id = client.create_invoice_signed(
        &manager,
        &merchant,
        &description,
        &amount,
        &token,
        &nonce,
        &signature,
    );

    assert!(invoice_id > 0, "Invoice ID should be positive");

    // Verify invoice data integrity
    let invoice = client.get_invoice(&invoice_id);
    assert_eq!(invoice.amount, amount);
    assert_eq!(invoice.merchant_id, 1);
}

/// Test 2: Invalid signature – tampering with the amount causes verification failure.
#[test]
#[should_panic(expected = "HostError: Error(Crypto, InvalidInput)")]
fn test_invalid_signature_tampered_amount() {
    let (env, client, contract_id, admin) = setup_test();

    let manager = Address::generate(&env);
    client.grant_role(&admin, &manager, &Role::Manager);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let keypair = generate_keypair();
    let pub_key = BytesN::from_array(&env, &keypair.public_key_bytes);
    client.set_merchant_key(&merchant, &pub_key);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Tampered Amount");
    let original_amount: i128 = 1000;
    let tampered_amount: i128 = 9999;
    let nonce = create_nonce(&env, 1);

    // Sign with the original amount
    let signature = sign_invoice(
        &env,
        &contract_id,
        &keypair,
        &merchant,
        &description,
        original_amount,
        &token,
        &nonce,
    );

    // Submit with the tampered amount → must panic
    client.create_invoice_signed(
        &manager,
        &merchant,
        &description,
        &tampered_amount,
        &token,
        &nonce,
        &signature,
    );
}

/// Test 3: Invalid signature – tampering with the description causes verification failure.
#[test]
#[should_panic(expected = "HostError: Error(Crypto, InvalidInput)")]
fn test_invalid_signature_tampered_description() {
    let (env, client, contract_id, admin) = setup_test();

    let manager = Address::generate(&env);
    client.grant_role(&admin, &manager, &Role::Manager);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let keypair = generate_keypair();
    let pub_key = BytesN::from_array(&env, &keypair.public_key_bytes);
    client.set_merchant_key(&merchant, &pub_key);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let original_desc = String::from_str(&env, "Original description");
    let tampered_desc = String::from_str(&env, "Tampered description");
    let amount: i128 = 1000;
    let nonce = create_nonce(&env, 1);

    // Sign with the original description
    let signature = sign_invoice(
        &env,
        &contract_id,
        &keypair,
        &merchant,
        &original_desc,
        amount,
        &token,
        &nonce,
    );

    // Submit with the tampered description → must panic
    client.create_invoice_signed(
        &manager,
        &merchant,
        &tampered_desc,
        &amount,
        &token,
        &nonce,
        &signature,
    );
}

/// Test 4: Replay attack – using the same nonce twice is blocked.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #14)")]
fn test_replay_attack_same_nonce() {
    let (env, client, contract_id, admin) = setup_test();

    let manager = Address::generate(&env);
    client.grant_role(&admin, &manager, &Role::Manager);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let keypair = generate_keypair();
    let pub_key = BytesN::from_array(&env, &keypair.public_key_bytes);
    client.set_merchant_key(&merchant, &pub_key);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Replay Attack Test");
    let amount: i128 = 2000;
    let nonce = create_nonce(&env, 42);

    let signature = sign_invoice(
        &env,
        &contract_id,
        &keypair,
        &merchant,
        &description,
        amount,
        &token,
        &nonce,
    );

    // First call succeeds
    client.create_invoice_signed(
        &manager,
        &merchant,
        &description,
        &amount,
        &token,
        &nonce,
        &signature,
    );

    // Second call with the same nonce → NonceAlreadyUsed (#14)
    client.create_invoice_signed(
        &manager,
        &merchant,
        &description,
        &amount,
        &token,
        &nonce,
        &signature,
    );
}

/// Test 5: Wrong merchant – Merchant A's signature cannot authorise an invoice for Merchant B.
#[test]
#[should_panic(expected = "HostError: Error(Crypto, InvalidInput)")]
fn test_wrong_merchant_signature() {
    let (env, client, contract_id, admin) = setup_test();

    let manager = Address::generate(&env);
    client.grant_role(&admin, &manager, &Role::Manager);

    let merchant_a = Address::generate(&env);
    let merchant_b = Address::generate(&env);
    client.register_merchant(&merchant_a);
    client.register_merchant(&merchant_b);

    let keypair_a = generate_keypair();
    let keypair_b = generate_keypair();
    client.set_merchant_key(
        &merchant_a,
        &BytesN::from_array(&env, &keypair_a.public_key_bytes),
    );
    client.set_merchant_key(
        &merchant_b,
        &BytesN::from_array(&env, &keypair_b.public_key_bytes),
    );

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Wrong Merchant Test");
    let amount: i128 = 3000;
    let nonce = create_nonce(&env, 1);

    // Sign as Merchant A
    let signature_a = sign_invoice(
        &env,
        &contract_id,
        &keypair_a,
        &merchant_a,
        &description,
        amount,
        &token,
        &nonce,
    );

    // Submit for Merchant B using Merchant A's signature → crypto error
    client.create_invoice_signed(
        &manager,
        &merchant_b,
        &description,
        &amount,
        &token,
        &nonce,
        &signature_a,
    );
}

/// Test 6: No public key – attempting to use a signature for a merchant
/// who has not registered a key panics with MerchantKeyNotFound (#11).
#[test]
#[should_panic(expected = "HostError: Error(Contract, #11)")]
fn test_no_public_key() {
    let (env, client, _contract_id, admin) = setup_test();

    let manager = Address::generate(&env);
    client.grant_role(&admin, &manager, &Role::Manager);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);
    // Deliberately do NOT set a merchant key

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "No Key Test");
    let amount: i128 = 1000;
    let nonce = create_nonce(&env, 1);

    // Dummy signature – the error fires before crypto verification
    let sig_bytes: [u8; 64] = [0; 64];
    let signature = BytesN::from_array(&env, &sig_bytes);

    client.create_invoice_signed(
        &manager,
        &merchant,
        &description,
        &amount,
        &token,
        &nonce,
        &signature,
    );
}

/// Test 7: Nonce independence – nonces are scoped per-merchant.
/// Merchant A using Nonce X does NOT prevent Merchant B from using the same Nonce X.
#[test]
fn test_nonce_independence_per_merchant() {
    let (env, client, contract_id, admin) = setup_test();

    let manager = Address::generate(&env);
    client.grant_role(&admin, &manager, &Role::Manager);

    let merchant_a = Address::generate(&env);
    let merchant_b = Address::generate(&env);
    client.register_merchant(&merchant_a);
    client.register_merchant(&merchant_b);

    let keypair_a = generate_keypair();
    let keypair_b = generate_keypair();
    client.set_merchant_key(
        &merchant_a,
        &BytesN::from_array(&env, &keypair_a.public_key_bytes),
    );
    client.set_merchant_key(
        &merchant_b,
        &BytesN::from_array(&env, &keypair_b.public_key_bytes),
    );

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Nonce Independence");
    let amount: i128 = 1000;

    // Both merchants use the SAME nonce value
    let shared_nonce = create_nonce(&env, 99);

    // Merchant A signs and uses the nonce
    let sig_a = sign_invoice(
        &env,
        &contract_id,
        &keypair_a,
        &merchant_a,
        &description,
        amount,
        &token,
        &shared_nonce,
    );
    let id_a = client.create_invoice_signed(
        &manager,
        &merchant_a,
        &description,
        &amount,
        &token,
        &shared_nonce,
        &sig_a,
    );

    // Merchant B signs the same nonce – should still succeed
    let sig_b = sign_invoice(
        &env,
        &contract_id,
        &keypair_b,
        &merchant_b,
        &description,
        amount,
        &token,
        &shared_nonce,
    );
    let id_b = client.create_invoice_signed(
        &manager,
        &merchant_b,
        &description,
        &amount,
        &token,
        &shared_nonce,
        &sig_b,
    );

    assert!(id_a > 0);
    assert!(id_b > id_a);
}
