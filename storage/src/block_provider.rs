use crate::bytes::Bytes;
use crate::chain::{IndexedBlock, IndexedBlockHeader, IndexedTransaction};
use crate::{BlockMeta, BlockRef};
use bitcrypto::SHA256D;

pub trait BlockHeaderProvider {
	/// resolves header bytes by block reference (number/hash)
	fn block_header_bytes(&self, block_ref: BlockRef) -> Option<Bytes>;

	/// resolves header bytes by block reference (number/hash)
	fn block_header(&self, block_ref: BlockRef) -> Option<IndexedBlockHeader>;
}

pub trait BlockProvider: BlockHeaderProvider {
	/// resolves block meta data
	fn block_meta(&self, block_ref: BlockRef) -> Option<BlockMeta>;

	/// resolves hash by block number
	fn block_hash(&self, number: u32) -> Option<SHA256D>;

	/// resolves block number by block hash
	fn block_number(&self, hash: &SHA256D) -> Option<u32>;

	/// resolves deserialized block body by block reference (number/hash)
	fn block(&self, block_ref: BlockRef) -> Option<IndexedBlock>;

	/// returns true if store contains given block
	fn contains_block(&self, block_ref: BlockRef) -> bool {
		self.block_header_bytes(block_ref).is_some()
	}

	/// resolves list of block transactions by block reference (number/hash)
	fn block_transaction_hashes(&self, block_ref: BlockRef) -> Vec<SHA256D>;

	/// returns all transactions in the block by block reference (number/hash)
	fn block_transactions(&self, block_ref: BlockRef) -> Vec<IndexedTransaction>;
}
