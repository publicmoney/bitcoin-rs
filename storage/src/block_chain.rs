use crate::chain::{IndexedBlock, IndexedBlockHeader};
use crate::{BlockOrigin, Error, SideChainOrigin, Store};
use bitcrypto::SHA256D;

pub trait ForkChain {
	/// Returns forks underlaying store.
	fn store(&self) -> &dyn Store;

	/// Flush fork changes to canon chain.
	/// Should not be used directly from outside of `BlockChain`.
	fn flush(&self) -> Result<(), Error>;
}

pub trait BlockChain {
	/// Inserts new block into blockchain
	fn insert(&self, block: IndexedBlock) -> Result<(), Error>;

	/// Canonizes block with given hash
	fn canonize(&self, block_hash: &SHA256D) -> Result<(), Error>;

	/// Decanonizes best block
	fn decanonize(&self) -> Result<SHA256D, Error>;

	fn flush(&self) -> Result<(), Error>;

	/// Checks block origin
	fn block_origin(&self, header: &IndexedBlockHeader) -> Result<BlockOrigin, Error>;
}

pub trait Forkable {
	/// Forks current blockchain.
	/// Lifetime guarantees fork relationship with canon chain.
	fn fork<'a>(&'a self, side_chain: SideChainOrigin) -> Result<Box<dyn ForkChain + 'a>, Error>;

	/// Switches blockchain to given fork.
	/// Lifetime guarantees that fork comes from this canon chain.
	fn switch_to_fork<'a>(&'a self, fork: Box<dyn ForkChain + 'a>) -> Result<(), Error>;
}
