#![cfg(test)]
extern crate alloc;

use crate::shade::{Shade, ShadeClient};
use crate::types::Role;
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{Address, Bytes, BytesN, Env, String};

fn setup_test() -> (Env, ShadeClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, contract_id, admin)
}

/// Build the same message as signature_util::build_message
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

fn create_nonce(env: &Env) -> BytesN<32> {
    let mut nonce: [u8; 32] = [0; 32];
    for (i, item) in nonce.iter_mut().enumerate() {
        *item = (i as u8).wrapping_add(1);
    }
    BytesN::from_array(env, &nonce)
}

fn create_unique_nonce(env: &Env, seed: u8) -> BytesN<32> {
    let mut nonce: [u8; 32] = [0; 32];
    for (i, item) in nonce.iter_mut().enumerate() {
        *item = (i as u8).wrapping_add(seed);
    }
    BytesN::from_array(env, &nonce)
}

/// Test Case 1: Manager Path - Successful Signed Invoice Creation
#[test]
fn test_create_invoice_signed_manager_success() {
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
    let description = String::from_str(&env, "Signed Invoice");
    let amount: i128 = 1000;
    let nonce = create_nonce(&env);

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

    assert!(invoice_id > 0, "Invoice should be created with valid ID");

    let invoice = client.get_invoice(&invoice_id);
    assert_eq!(invoice.amount, amount);
    assert_eq!(invoice.merchant_id, 1);
}

/// Test Case 2: Admin Path - Successful Signed Invoice Creation
#[test]
fn test_create_invoice_signed_admin_success() {
    let (env, client, contract_id, admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let keypair = generate_keypair();
    let pub_key = BytesN::from_array(&env, &keypair.public_key_bytes);
    client.set_merchant_key(&merchant, &pub_key);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Signed Invoice by Admin");
    let amount: i128 = 5000;
    let nonce = create_nonce(&env);

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
        &admin,
        &merchant,
        &description,
        &amount,
        &token,
        &nonce,
        &signature,
    );

    assert!(invoice_id > 0, "Invoice should be created with valid ID");

    let invoice = client.get_invoice(&invoice_id);
    assert_eq!(invoice.amount, amount);
}

/// Test Case 3: Role Enforcement - Guest Cannot Call create_invoice_signed
#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_create_invoice_signed_guest_unauthorized() {
    let (env, client, contract_id, admin) = setup_test();

    let guest = Address::generate(&env);
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let keypair = generate_keypair();
    let pub_key = BytesN::from_array(&env, &keypair.public_key_bytes);
    client.set_merchant_key(&merchant, &pub_key);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Unauthorized Signed Invoice");
    let nonce = create_nonce(&env);
    let signature = sign_invoice(
        &env,
        &contract_id,
        &keypair,
        &merchant,
        &description,
        1000,
        &token,
        &nonce,
    );

    client.create_invoice_signed(
        &guest,
        &merchant,
        &description,
        &1000,
        &token,
        &nonce,
        &signature,
    );
}

/// Test Case 4: Role Enforcement - Operator Cannot Call create_invoice_signed
#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_create_invoice_signed_operator_unauthorized() {
    let (env, client, contract_id, admin) = setup_test();

    let operator = Address::generate(&env);
    client.grant_role(&admin, &operator, &Role::Operator);

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let keypair = generate_keypair();
    let pub_key = BytesN::from_array(&env, &keypair.public_key_bytes);
    client.set_merchant_key(&merchant, &pub_key);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Unauthorized Operators Invoice");
    let nonce = create_nonce(&env);
    let signature = sign_invoice(
        &env,
        &contract_id,
        &keypair,
        &merchant,
        &description,
        1000,
        &token,
        &nonce,
    );

    client.create_invoice_signed(
        &operator,
        &merchant,
        &description,
        &1000,
        &token,
        &nonce,
        &signature,
    );
}

/// Test Case 5: Invalid Amount Validation - Zero Amount
#[test]
#[should_panic(expected = "HostError: Error(Contract, #7)")]
fn test_create_invoice_signed_invalid_amount_zero() {
    let (env, client, _contract_id, admin) = setup_test();

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
    let description = String::from_str(&env, "Invalid Amount Invoice");
    let nonce = create_nonce(&env);
    // Dummy signature — validation fails before reaching signature check
    let sig_bytes: [u8; 64] = [0; 64];
    let signature = BytesN::from_array(&env, &sig_bytes);

    client.create_invoice_signed(
        &manager,
        &merchant,
        &description,
        &0,
        &token,
        &nonce,
        &signature,
    );
}

/// Test Case 6: Invalid Amount - Negative Amount
#[test]
#[should_panic(expected = "HostError: Error(Contract, #7)")]
fn test_create_invoice_signed_invalid_amount_negative() {
    let (env, client, _contract_id, admin) = setup_test();

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
    let description = String::from_str(&env, "Negative Amount Invoice");
    let nonce = create_nonce(&env);
    let sig_bytes: [u8; 64] = [0; 64];
    let signature = BytesN::from_array(&env, &sig_bytes);

    client.create_invoice_signed(
        &manager,
        &merchant,
        &description,
        &-1000,
        &token,
        &nonce,
        &signature,
    );
}

/// Test Case 7: Merchant Validation - Unregistered Merchant
#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_create_invoice_signed_unregistered_merchant() {
    let (env, client, _contract_id, admin) = setup_test();

    let manager = Address::generate(&env);
    client.grant_role(&admin, &manager, &Role::Manager);

    let unregistered_merchant = Address::generate(&env);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Unknown Merchant Invoice");
    let nonce = create_nonce(&env);
    let sig_bytes: [u8; 64] = [0; 64];
    let signature = BytesN::from_array(&env, &sig_bytes);

    client.create_invoice_signed(
        &manager,
        &unregistered_merchant,
        &description,
        &1000,
        &token,
        &nonce,
        &signature,
    );
}

/// Test Case 8: End-to-End Integration - Multiple Invoices via Signed Path
#[test]
fn test_create_invoice_signed_multiple_invoices() {
    let (env, client, contract_id, admin) = setup_test();

    let manager = Address::generate(&env);
    client.grant_role(&admin, &manager, &Role::Manager);

    let merchant1 = Address::generate(&env);
    let merchant2 = Address::generate(&env);

    client.register_merchant(&merchant1);
    client.register_merchant(&merchant2);

    let keypair1 = generate_keypair();
    let keypair2 = generate_keypair();

    client.set_merchant_key(
        &merchant1,
        &BytesN::from_array(&env, &keypair1.public_key_bytes),
    );
    client.set_merchant_key(
        &merchant2,
        &BytesN::from_array(&env, &keypair2.public_key_bytes),
    );

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.add_accepted_token(&admin, &token);
    let description = String::from_str(&env, "Multi Invoice Test");

    let nonce1 = create_unique_nonce(&env, 1);
    let sig1 = sign_invoice(
        &env,
        &contract_id,
        &keypair1,
        &merchant1,
        &description,
        1000,
        &token,
        &nonce1,
    );
    let invoice_id_1 = client.create_invoice_signed(
        &manager,
        &merchant1,
        &description,
        &1000,
        &token,
        &nonce1,
        &sig1,
    );

    let nonce2 = create_unique_nonce(&env, 2);
    let sig2 = sign_invoice(
        &env,
        &contract_id,
        &keypair2,
        &merchant2,
        &description,
        2000,
        &token,
        &nonce2,
    );
    let invoice_id_2 = client.create_invoice_signed(
        &manager,
        &merchant2,
        &description,
        &2000,
        &token,
        &nonce2,
        &sig2,
    );

    let nonce3 = create_unique_nonce(&env, 3);
    let sig3 = sign_invoice(
        &env,
        &contract_id,
        &keypair1,
        &merchant1,
        &description,
        3000,
        &token,
        &nonce3,
    );
    let invoice_id_3 = client.create_invoice_signed(
        &manager,
        &merchant1,
        &description,
        &3000,
        &token,
        &nonce3,
        &sig3,
    );

    assert!(invoice_id_1 > 0);
    assert!(invoice_id_2 > invoice_id_1);
    assert!(invoice_id_3 > invoice_id_2);

    let inv1 = client.get_invoice(&invoice_id_1);
    let inv2 = client.get_invoice(&invoice_id_2);
    let inv3 = client.get_invoice(&invoice_id_3);

    assert_eq!(inv1.amount, 1000);
    assert_eq!(inv2.amount, 2000);
    assert_eq!(inv3.amount, 3000);
    assert_eq!(inv1.merchant_id, 1);
    assert_eq!(inv2.merchant_id, 2);
    assert_eq!(inv3.merchant_id, 1);
}
