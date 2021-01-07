use crate::ham_db::PRef;
use bitcrypto::sha256;
use chain::{TransactionInput, TransactionOutput};

// Keyed by block hash.
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct DbBlock {
	pub header: PRef,
	pub meta: PRef,
	pub txs: Vec<PRef>, // PRefs for DbTransaction
}

// Keyed by transaction hash.
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct DbTransaction {
	pub tx_pref: PRef,
	pub meta_pref: PRef,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct DbInputKey {
	pub hash_prefix: Vec<u8>,
	pub index: u16,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct DbOutputKey {
	script_hash_prefix: Vec<u8>,
}

impl DbInputKey {
	pub fn from(input: &TransactionInput) -> DbInputKey {
		DbInputKey {
			hash_prefix: input.previous_output.hash[0..7].to_vec(),
			index: input.previous_output.index as u16,
		}
	}
}

impl DbOutputKey {
	pub fn from(output: &TransactionOutput) -> DbOutputKey {
		DbOutputKey {
			script_hash_prefix: sha256(&output.script_pubkey[..]).to_vec(),
		}
	}
}
