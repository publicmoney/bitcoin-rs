use bitcrypto::sha256;
use chain::{TransactionInput, TransactionOutput};
use hammersbald::PRef;
use serialization::bytes::Bytes;
use serialization::serialize;

// Keyed by block hash.
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct DbBlock {
	pub header: PRef,
	pub meta: PRef,
	pub txs: Vec<PRef>, // PRefs for DbTransaction
}

//todo constructors

// Keyed by transaction hash.
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct DbTransaction {
	pub tx_pref: PRef,
	pub meta_pref: PRef,
}

impl DbTransaction {
	pub fn new(tx_pref: PRef, meta_pref: PRef) -> DbTransaction {
		DbTransaction { tx_pref, meta_pref }
	}
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

pub struct DbOutput {
	pub key: DbOutputKey,
	pub value: PRef,
}

impl DbOutput {
	pub fn from(output: &TransactionOutput, tx_ref: PRef) -> (Bytes, PRef) {
		(
			serialize(&DbOutputKey {
				script_hash_prefix: sha256(&output.script_pubkey[..]).to_vec(),
			}),
			tx_ref,
		)
	}
}

impl DbInputKey {
	pub fn from(input: &TransactionInput) -> Bytes {
		serialize(&DbInputKey {
			hash_prefix: input.previous_output.hash[0..7].to_vec(),
			index: input.previous_output.index as u16,
		})
	}
}
