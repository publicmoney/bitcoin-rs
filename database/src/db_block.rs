use crate::adapator::BlockChainDatabase;
use bit_vec::BitVec;
use bitcrypto::{sha256, SHA256D};
use chain::compact::Compact;
use chain::{BlockHeader, IndexedTransaction, Transaction, TransactionInput, TransactionOutput};
use hammersbald::PRef;
use std::error::Error;

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct DbBlockHeader {
	pub version: u32,
	pub previous_header_hash: PRef, // 24 MB saved
	pub merkle_root_hash: SHA256D,
	pub time: u32,
	pub bits: Compact,
	pub txs: PRef,
	pub nonce: u32,
	pub number: u32,
	pub n_txs: u32,
	pub n_chain_txs: u32,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct DbTransaction {
	pub version: i32,
	pub inputs: Vec<TransactionInput>,
	pub outputs: Vec<TransactionOutput>,
	pub lock_time: u32,
	pub height: PRef, // todo where is this going to be used?
	pub bits: BitVec,
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
	pub fn from(output: TransactionOutput, tx_ref: PRef) -> DbOutput {
		DbOutput {
			key: DbOutputKey {
				script_hash_prefix: sha256(&output.script_pubkey[..]).to_vec(),
			},
			value: tx_ref,
		}
	}
}
impl DbInputKey {
	pub fn from(input: &TransactionInput) -> DbInputKey {
		DbInputKey {
			hash_prefix: input.previous_output.hash[0..7].to_vec(),
			index: input.previous_output.index as u16,
		}
	}
}

impl DbTransaction {
	pub fn from(t: &IndexedTransaction) -> DbTransaction {
		DbTransaction {
			version: t.raw.version,
			lock_time: t.raw.lock_time,
			inputs: t.raw.inputs.clone(),
			outputs: t.raw.outputs.clone(),
			height: PRef::default(),
			bits: BitVec::from_elem(t.raw.outputs.len() + 1, false),
		}
	}

	pub fn to_indexed(&self) -> IndexedTransaction {
		Transaction {
			version: self.version,
			lock_time: self.lock_time,
			inputs: self.inputs.clone(),
			outputs: self.outputs.clone(),
		}
		.into()
	}

	/// Denote particular output as used
	pub fn denote_used(&mut self, index: usize) {
		self.bits.set(index + 1, true);
	}

	/// Denote particular output as not used
	pub fn denote_unused(&mut self, index: usize) {
		self.bits.set(index + 1, false);
	}

	pub fn set_coinbase(&mut self) {
		self.bits.set(0, true);
	}
}

impl DbBlockHeader {
	pub fn from(block: &chain::IndexedBlock, adaptor: &BlockChainDatabase) -> Result<DbBlockHeader, storage::Error> {
		Ok(DbBlockHeader {
			version: block.header.raw.version,
			previous_header_hash: adaptor.get_ref(&block.header.raw.previous_header_hash)?.unwrap_or_default(),
			merkle_root_hash: block.header.raw.merkle_root_hash,
			time: block.header.raw.time,
			bits: block.header.raw.bits,
			nonce: block.header.raw.nonce,
			txs: PRef::default(),
			number: 0,
			n_txs: block.transactions.len() as u32,
			n_chain_txs: 0,
		})
	}

	pub fn to_block(&self, adaptor: &BlockChainDatabase) -> BlockHeader {
		BlockHeader {
			version: self.version,
			nonce: self.nonce,
			bits: self.bits,
			time: self.time,
			merkle_root_hash: self.merkle_root_hash,
			previous_header_hash: adaptor.get_key(self.previous_header_hash).unwrap_or_default(),
		}
	}
}
