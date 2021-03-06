use crate::block::Block;
use crate::indexed_header::IndexedBlockHeader;
use crate::indexed_transaction::IndexedTransaction;
use crate::merkle_root::merkle_root;
use crate::transaction::Transaction;
use bitcrypto::{FromHex, SHA256D};
use ser::{deserialize, serialized_list_size, serialized_list_size_with_flags, Serializable, SERIALIZE_TRANSACTION_WITNESS};
use std::cmp;

#[derive(Debug, Clone, Deserializable)]
pub struct IndexedBlock {
	pub header: IndexedBlockHeader,
	pub transactions: Vec<IndexedTransaction>,
}

#[cfg(feature = "test-helpers")]
impl From<Block> for IndexedBlock {
	fn from(block: Block) -> Self {
		Self::from_raw(block)
	}
}
impl cmp::PartialEq for IndexedBlock {
	fn eq(&self, other: &Self) -> bool {
		self.header.hash == other.header.hash
	}
}

impl IndexedBlock {
	pub fn new(header: IndexedBlockHeader, transactions: Vec<IndexedTransaction>) -> Self {
		IndexedBlock { header, transactions }
	}

	/// Explicit conversion of the raw Block into IndexedBlock.
	///
	/// Hashes block header + transactions.
	pub fn from_raw(block: Block) -> Self {
		let Block {
			block_header,
			transactions,
		} = block;
		Self::new(
			IndexedBlockHeader::from_raw(block_header),
			transactions.into_iter().map(IndexedTransaction::from_raw).collect(),
		)
	}

	pub fn hash(&self) -> &SHA256D {
		&self.header.hash
	}

	pub fn to_raw_block(self) -> Block {
		Block::new(self.header.raw, self.transactions.into_iter().map(|tx| tx.raw).collect())
	}

	pub fn size(&self) -> usize {
		let header_size = self.header.raw.serialized_size();
		let transactions = self.transactions.iter().map(|tx| &tx.raw).collect::<Vec<_>>();
		let txs_size = serialized_list_size::<Transaction, &Transaction>(&transactions);
		header_size + txs_size
	}

	pub fn size_with_witness(&self) -> usize {
		let header_size = self.header.raw.serialized_size();
		let transactions = self.transactions.iter().map(|tx| &tx.raw).collect::<Vec<_>>();
		let txs_size = serialized_list_size_with_flags::<Transaction, &Transaction>(&transactions, SERIALIZE_TRANSACTION_WITNESS);
		header_size + txs_size
	}

	pub fn weight(&self) -> usize {
		let size = self.size();
		let size_with_witness = self.size_with_witness();
		(size * 3) + size_with_witness
	}

	pub fn merkle_root(&self) -> SHA256D {
		merkle_root(&self.transactions.iter().map(|tx| tx.hash).collect::<Vec<SHA256D>>())
	}

	pub fn witness_merkle_root(&self) -> SHA256D {
		let hashes = match self.transactions.split_first() {
			None => vec![],
			Some((_, rest)) => {
				let mut hashes = vec![SHA256D::default()];
				hashes.extend(rest.iter().map(|tx| tx.raw.witness_hash()));
				hashes
			}
		};
		merkle_root(&hashes)
	}

	pub fn is_final(&self, height: u32) -> bool {
		self.transactions
			.iter()
			.all(|tx| tx.raw.is_final_in_block(height, self.header.raw.time))
	}
}

impl From<&'static str> for IndexedBlock {
	fn from(s: &'static str) -> Self {
		let hex: Vec<u8> = FromHex::from_hex(s).unwrap();
		deserialize(&*hex).unwrap()
	}
}

#[cfg(test)]
mod tests {
	use super::IndexedBlock;

	#[test]
	fn size_without_witness() {
		let block_without_witness: IndexedBlock = "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000".into();

		assert_eq!(block_without_witness.size(), 132);
		assert_eq!(block_without_witness.size_with_witness(), 132);
		assert_eq!(block_without_witness.weight(), 528);
	}

	#[test]
	fn size_with_witness() {
		// bip143 block
		let block_with_witness: IndexedBlock = "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000010100000000000000000000000000000000000000000000000000000000000000000000000000000000000001010000000000".into();

		assert_eq!(block_with_witness.size(), 132);
		assert_eq!(block_with_witness.size_with_witness(), 137);
		assert_eq!(block_with_witness.weight(), 533);
	}
}
