use std::collections::HashMap;

use serde::{Serialize, Deserialize};
use aptos_crypto::ed25519::{Ed25519PrivateKey, Ed25519PublicKey, Ed25519Signature};
use aptos_crypto_derive::{BCSCryptoHash, CryptoHasher};

use serde_json::value::Value;
use rust_rapidsnark::FullProver;
use aptos_types::jwks::rsa::RSA_JWK;
use anyhow::anyhow;

use crate::{metrics, input_conversion::rsa::RsaPublicKey};


// TODO separate into:
// - api.rs
// - config.rs
//   - Put constants for "config.yml" in config.rs
//   - i.e., path

#[derive(Debug, Serialize, Deserialize)]
pub struct ProverServerConfig {
    pub zkey_path: String,
    pub witness_gen_binary_folder_path: String,
    pub test_verification_key_path: String,
    pub oidc_providers: Vec<OidcProvider>,
    pub jwk_refresh_rate_secs: u64,
    pub port: u16,
    pub metrics_port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OidcProvider {
    pub iss: String,
    pub endpoint_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProverServerSecrets {
    pub private_key : Ed25519PrivateKey,
}

pub struct ProverServerState {
    pub full_prover: FullProver,
    pub public_key : Ed25519PublicKey,
    pub private_key : Ed25519PrivateKey,
    pub metrics: metrics::ProverServerMetrics
}
