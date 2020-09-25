use crate::BlockHeight;
use crate::{
	BlockChain, BlockHeaderProvider, BlockProvider, Forkable, TransactionMetaProvider, TransactionOutputProvider, TransactionProvider,
};
use chain::IndexedBlockHeader;
use std::sync::Arc;

pub trait CanonStore: Store + Forkable {
	fn as_store(&self) -> &dyn Store;
}

/// Blockchain storage interface
pub trait Store: AsSubstore {
	fn best_block(&self) -> BlockHeight;

	fn best_header(&self) -> IndexedBlockHeader;

	fn difficulty(&self) -> f64;

	fn stats(&self);

	fn size(&self) -> u64;

	fn shutdown(&self);
}

/// Allows casting Arc<Store> to reference to any substore type
pub trait AsSubstore: BlockChain + BlockProvider + TransactionProvider + TransactionMetaProvider + TransactionOutputProvider {
	fn as_block_provider(&self) -> &dyn BlockProvider;

	fn as_block_header_provider(&self) -> &dyn BlockHeaderProvider;

	fn as_transaction_provider(&self) -> &dyn TransactionProvider;

	fn as_transaction_output_provider(&self) -> &dyn TransactionOutputProvider;

	fn as_transaction_meta_provider(&self) -> &dyn TransactionMetaProvider;
}

impl<T> AsSubstore for T
where
	T: BlockChain + BlockProvider + TransactionProvider + TransactionMetaProvider + TransactionOutputProvider,
{
	fn as_block_provider(&self) -> &dyn BlockProvider {
		&*self
	}

	fn as_block_header_provider(&self) -> &dyn BlockHeaderProvider {
		&*self
	}

	fn as_transaction_provider(&self) -> &dyn TransactionProvider {
		&*self
	}

	fn as_transaction_output_provider(&self) -> &dyn TransactionOutputProvider {
		&*self
	}

	fn as_transaction_meta_provider(&self) -> &dyn TransactionMetaProvider {
		&*self
	}
}

pub type SharedStore = Arc<dyn CanonStore + Send + Sync>;
