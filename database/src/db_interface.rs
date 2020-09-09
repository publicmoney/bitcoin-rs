use bitcrypto::SHA256D;
use chain::{IndexedBlock, IndexedBlockHeader, IndexedTransaction};
use storage::{BlockHeight, BlockMeta, TransactionMeta};

pub trait DbInterface: Send + Sync {
	fn insert_block(&self, block: IndexedBlock) -> Result<(), storage::Error>;

	fn fetch_block(&self, block_hash: &SHA256D) -> Result<Option<IndexedBlock>, storage::Error>;

	fn fetch_block_header(&self, block_hash: &SHA256D) -> Result<Option<IndexedBlockHeader>, storage::Error>;

	fn fetch_block_meta(&self, block_hash: &SHA256D) -> Result<Option<BlockMeta>, storage::Error>;

	fn fetch_transactions(&self, block_hash: &SHA256D) -> Result<Option<Vec<IndexedTransaction>>, storage::Error>;

	fn fetch_transaction_hashes(&self, block_hash: &SHA256D) -> Result<Option<Vec<SHA256D>>, storage::Error>;

	fn fetch_transaction(&self, tx_hash: &SHA256D) -> Result<Option<IndexedTransaction>, storage::Error>;

	fn fetch_transaction_meta(&self, tx_hash: &SHA256D) -> Result<Option<TransactionMeta>, storage::Error>;

	fn update_block_meta(&self, block_hash: &SHA256D, block_meta: &BlockMeta) -> Result<(), storage::Error>;

	fn update_transaction_meta(&self, tx_hash: &SHA256D, tx_meta: TransactionMeta) -> Result<(), storage::Error>;

	fn set_block_by_number(&self, block_hash: &SHA256D, number: u32) -> Result<(), storage::Error>;

	fn block_hash(&self, number: u32) -> Result<Option<SHA256D>, storage::Error>;

	fn best_block(&self) -> Result<BlockHeight, storage::Error>;

	fn set_best(&self, block_hash: &SHA256D) -> Result<(), storage::Error>;

	fn flush(&self) -> Result<(), storage::Error>;

	fn info(&self) -> Result<(), storage::Error>;

	fn shutdown(&self) -> Result<(), storage::Error>;
}
