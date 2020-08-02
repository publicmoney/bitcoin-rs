use crate::db_interface::DbInterface;
use crate::ham_db::HamDb;
use crate::overlay_db::OverlayDatabase;
use bitcrypto::SHA256D;
use chain::{IndexedBlock, IndexedBlockHeader, IndexedTransaction, OutPoint, TransactionOutput};
use parking_lot::RwLock;
use serialization::serialize;
use std::collections::HashMap;
use storage::bytes::Bytes;
use storage::{
	BlockChain, BlockHeaderProvider, BlockHeight, BlockMeta, BlockOrigin, BlockProvider, BlockRef, ForkChain, SideChainOrigin, Store,
	TransactionMeta, TransactionMetaProvider, TransactionOutputProvider, TransactionProvider,
};

const MAX_FORK_ROUTE_PRESET: usize = 2048;

pub struct BlockChainDatabase<T>
where
	T: DbInterface,
{
	best_block: RwLock<BlockHeight>,
	db: T,
}

pub struct ForkChainDatabase<'a, T>
where
	T: DbInterface,
{
	blockchain: BlockChainDatabase<OverlayDatabase<'a, T>>,
}

impl<'a, T: DbInterface> ForkChain for ForkChainDatabase<'a, T> {
	fn store(&self) -> &dyn Store {
		&self.blockchain
	}

	fn flush(&self) -> Result<(), storage::Error> {
		self.blockchain.db.flush()
	}
}

pub fn from_ham(e: hammersbald::Error) -> storage::Error {
	storage::Error::DatabaseError(e.to_string())
}

pub fn from_serial(e: serialization::Error) -> storage::Error {
	storage::Error::DatabaseError(e.to_string())
}

impl BlockChainDatabase<HamDb> {
	pub fn transient() -> Result<BlockChainDatabase<HamDb>, storage::Error> {
		BlockChainDatabase::open(HamDb::transient()?)
	}

	pub fn persistent() -> Result<BlockChainDatabase<HamDb>, storage::Error> {
		BlockChainDatabase::open(HamDb::persistent()?)
	}
}

impl<T> BlockChainDatabase<T>
where
	T: DbInterface,
{
	pub fn open(db: T) -> Result<BlockChainDatabase<T>, storage::Error> {
		let best_block = db.best_block()?;
		Ok(BlockChainDatabase {
			db,
			best_block: RwLock::new(best_block),
		})
	}

	pub fn fork(&self, side_chain: SideChainOrigin) -> Result<ForkChainDatabase<T>, storage::Error> {
		let memory_db = BlockChainDatabase::open(OverlayDatabase::new(&self.db))?;

		for hash in side_chain.decanonized_route.into_iter().rev() {
			let decanonized_hash = memory_db.decanonize()?;
			assert_eq!(hash, decanonized_hash);
		}

		for block_hash in &side_chain.canonized_route {
			memory_db.canonize(block_hash)?;
		}

		let fork = ForkChainDatabase { blockchain: memory_db };

		Ok(fork)
	}

	pub fn switch_to_fork(&self, fork: ForkChainDatabase<T>) -> Result<(), storage::Error> {
		let mut best_block = self.best_block.write();
		*best_block = fork.blockchain.best_block.read().clone();
		fork.blockchain.db.flush()
	}

	pub fn insert(&self, block: IndexedBlock) -> Result<(), storage::Error> {
		if self.contains_block(block.hash().clone().into()) {
			return Ok(());
		}

		let parent_hash = block.header.raw.previous_header_hash.clone();
		if parent_hash != SHA256D::default() && !self.contains_block(parent_hash.clone().into()) {
			return Err(storage::Error::UnknownParent);
		}

		self.db.insert_block(block)?;
		self.db.flush()
	}

	pub fn block_origin(&self, header: &IndexedBlockHeader) -> Result<BlockOrigin, storage::Error> {
		let best_block = self.best_block.read();

		assert_eq!(Some(best_block.hash), self.block_hash(best_block.number));
		if self.contains_block(header.hash.clone().into()) {
			// it does not matter if it's canon chain or side chain block
			return Ok(BlockOrigin::KnownBlock);
		}

		if best_block.hash == header.raw.previous_header_hash {
			return Ok(BlockOrigin::CanonChain {
				block_number: best_block.number + 1,
			});
		}

		if !self.contains_block(header.raw.previous_header_hash.clone().into()) {
			return Err(storage::Error::UnknownParent);
		}

		let mut sidechain_route = Vec::new();
		let mut next_hash = header.raw.previous_header_hash.clone();

		for fork_len in 0..MAX_FORK_ROUTE_PRESET {
			match self.block_number(&next_hash) {
				Some(number) => {
					let block_number = number + fork_len as u32 + 1;
					let origin = SideChainOrigin {
						ancestor: number,
						canonized_route: sidechain_route.into_iter().rev().collect(),
						decanonized_route: (number + 1..best_block.number + 1)
							.into_iter()
							.filter_map(|decanonized_bn| self.block_hash(decanonized_bn))
							.collect(),
						block_number,
					};
					return if block_number > best_block.number {
						Ok(BlockOrigin::SideChainBecomingCanonChain(origin))
					} else {
						Ok(BlockOrigin::SideChain(origin))
					};
				}
				None => {
					sidechain_route.push(next_hash);
					next_hash = self
						.block_header(BlockRef::Hash(next_hash))
						.expect("not to find orphaned side chain in database; qed")
						.raw
						.previous_header_hash;
				}
			}
		}

		Err(storage::Error::AncientFork)
	}

	pub fn canonize(&self, block_hash: &SHA256D) -> Result<(), storage::Error> {
		// println!("Try to canonize {:?}", block_hash);
		let mut best_height = self.best_block.write();

		let new_best_block = match self.db.fetch_block(block_hash)? {
			Some(b) => b,
			_ => {
				error!(target: "db", "Block is not found during canonization: {}", block_hash);
				return Err(storage::Error::CannotCanonize);
			}
		};

		let mut best_block_meta = self.db.fetch_block_meta(&block_hash)?.unwrap();

		if best_height.hash == SHA256D::default() {
			// genesis block
			best_block_meta.number = 0;
			best_block_meta.n_chain_tx = new_best_block.transactions.len() as u32;
		} else {
			let current_db_block = self.db.fetch_block_meta(&best_height.hash)?.unwrap();
			best_block_meta.number = current_db_block.number + 1;
			best_block_meta.n_chain_tx = current_db_block.n_chain_tx + new_best_block.transactions.len() as u32;
		}

		let mut metas: HashMap<SHA256D, TransactionMeta> = HashMap::new();

		if let Some(tx) = new_best_block.transactions.first() {
			let mut meta = TransactionMeta::new(best_block_meta.number, tx.raw.outputs.len());
			meta.set_coinbase();
			metas.insert(tx.hash, meta);
		}

		for tx in new_best_block.transactions.iter().skip(1) {
			metas.insert(tx.hash, TransactionMeta::new(best_block_meta.number, tx.raw.outputs.len()));

			for input in &tx.raw.inputs {
				use std::collections::hash_map::Entry;

				match metas.entry(input.previous_output.hash) {
					Entry::Occupied(mut entry) => {
						let used_tx = entry.get_mut();
						used_tx.denote_used(input.previous_output.index as usize);
					}
					Entry::Vacant(entry) => {
						if let Some(mut used_tx_meta) = self.db.fetch_transaction_meta(&input.previous_output.hash)? {
							used_tx_meta.denote_used(input.previous_output.index as usize);
							entry.insert(used_tx_meta);
							continue;
						} else {
							error!(
								target: "db",
								"Cannot find tx during canonization of tx: {}/{}",
								input.previous_output.hash,
								input.previous_output.index,
							);
							return Err(storage::Error::CannotCanonize);
						}
					}
				}
			}
		}

		self.db.set_block_by_number(&new_best_block.header.hash, best_block_meta.number)?;
		self.db.set_best(&new_best_block.header.hash)?;
		self.db.update_block_meta(&new_best_block.header.hash, &best_block_meta)?;
		for (hash, meta) in metas {
			self.db.update_transaction_meta(&hash, meta)?;
		}
		self.db.flush()?;

		// println!("Canonized: {:?}", block_hash);
		*best_height = BlockHeight {
			hash: *block_hash,
			number: best_block_meta.number,
		};

		Ok(())
	}

	pub fn decanonize(&self) -> Result<SHA256D, storage::Error> {
		let mut best = self.best_block.write();
		let best_height = &best.clone();

		let best_block = match self.db.fetch_block(&best_height.hash)? {
			Some(block) => block,
			_ => {
				error!(target: "db", "Block is not found during decanonization: {}", best_height.hash);
				return Err(storage::Error::CannotDecanonize);
			}
		};

		let new_best_block = BlockHeight {
			hash: best_block.header.raw.previous_header_hash,
			number: if best_height.number > 0 { best_height.number - 1 } else { 0 },
		};

		trace!(target: "db", "decanonize, new best: {:?}", new_best_block);

		self.db.set_best(&best_block.header.raw.previous_header_hash)?;

		let mut metas: HashMap<SHA256D, TransactionMeta> = HashMap::new();

		for tx in best_block.transactions.iter().skip(1) {
			for input in &tx.raw.inputs {
				use std::collections::hash_map::Entry;

				match metas.entry(input.previous_output.hash) {
					Entry::Occupied(mut entry) => {
						let unused_tx = entry.get_mut();
						unused_tx.denote_unused(input.previous_output.index as usize);
					}
					Entry::Vacant(entry) => {
						if let Some(mut unused_tx) = self.db.fetch_transaction_meta(&input.previous_output.hash)? {
							unused_tx.denote_unused(input.previous_output.index as usize);
							entry.insert(unused_tx);
							continue;
						} else {
							error!(
								target: "db",
								"Cannot find tx meta during decanonization of tx {}/{}",
								input.previous_output.hash,
								input.previous_output.index,
							);
							return Err(storage::Error::CannotDecanonize);
						}
					}
				}
			}
		}

		for (hash, meta) in metas {
			self.db.update_transaction_meta(&hash, meta)?;
		}

		self.db.flush()?;
		*best = new_best_block;

		Ok(best_height.hash)
	}

	pub fn best_block(&self) -> BlockHeight {
		self.best_block.read().clone()
	}

	fn rollback_best(&self) -> Result<SHA256D, storage::Error> {
		unimplemented!()
	}

	fn resolve_hash(&self, block_ref: BlockRef) -> Option<SHA256D> {
		match block_ref {
			BlockRef::Number(n) => self.block_hash(n),
			BlockRef::Hash(h) => Some(h),
		}
	}
}

impl<T> BlockHeaderProvider for BlockChainDatabase<T>
where
	T: DbInterface,
{
	fn block_header_bytes(&self, block_ref: BlockRef) -> Option<Bytes> {
		self.block_header(block_ref).map(|header| serialize(&header.raw))
	}

	fn block_header(&self, block_ref: BlockRef) -> Option<IndexedBlockHeader> {
		self.resolve_hash(block_ref)
			.and_then(|block_hash| self.db.fetch_block_header(&block_hash).unwrap_or_default())
	}
}

impl<T> BlockProvider for BlockChainDatabase<T>
where
	T: DbInterface,
{
	fn block_meta(&self, block_ref: BlockRef) -> Option<BlockMeta> {
		self.resolve_hash(block_ref)
			.and_then(|hash| self.db.fetch_block_meta(&hash).unwrap())
	}

	fn block_hash(&self, number: u32) -> Option<SHA256D> {
		self.db.block_hash(number).unwrap_or_default()
	}

	fn block_number(&self, hash: &SHA256D) -> Option<u32> {
		self.db.fetch_block_meta(hash).unwrap_or_default().map(|meta| meta.number)
	}

	fn block(&self, block_ref: BlockRef) -> Option<IndexedBlock> {
		self.resolve_hash(block_ref)
			.and_then(|hash| self.db.fetch_block(&hash).unwrap_or_default())
	}

	fn block_transaction_hashes(&self, block_ref: BlockRef) -> Vec<SHA256D> {
		self.resolve_hash(block_ref)
			.and_then(|hash| self.db.fetch_transaction_hashes(&hash).unwrap_or_default())
			.unwrap_or_default()
	}

	fn block_transactions(&self, block_ref: BlockRef) -> Vec<IndexedTransaction> {
		self.block_transaction_hashes(block_ref)
			.into_iter()
			.filter_map(|hash| self.transaction(&hash))
			.collect()
	}
}

impl<T> Store for BlockChainDatabase<T>
where
	T: DbInterface,
{
	fn best_block(&self) -> BlockHeight {
		self.best_block()
	}

	/// get best header
	fn best_header(&self) -> IndexedBlockHeader {
		self.block_header(self.best_block().hash.into())
			.expect("best block header should be in db; qed")
	}

	/// get blockchain difficulty
	fn difficulty(&self) -> f64 {
		self.best_header().raw.bits.to_f64()
	}
}

impl<T> TransactionMetaProvider for BlockChainDatabase<T>
where
	T: DbInterface,
{
	fn transaction_meta(&self, hash: &SHA256D) -> Option<TransactionMeta> {
		self.db.fetch_transaction_meta(hash).unwrap_or_default()
	}
}

impl<T> TransactionProvider for BlockChainDatabase<T>
where
	T: DbInterface,
{
	fn transaction_bytes(&self, hash: &SHA256D) -> Option<Bytes> {
		self.transaction(hash).map(|tx| serialize(&tx.raw))
	}

	fn transaction(&self, hash: &SHA256D) -> Option<IndexedTransaction> {
		self.db.fetch_transaction(hash).unwrap_or_default()
	}
}

impl<T> TransactionOutputProvider for BlockChainDatabase<T>
where
	T: DbInterface,
{
	fn transaction_output(&self, prevout: &OutPoint, _transaction_index: usize) -> Option<TransactionOutput> {
		// return previous transaction outputs only for canon chain transactions
		self.transaction_meta(&prevout.hash)
			.and_then(|_| self.transaction(&prevout.hash))
			.and_then(|tx| tx.raw.outputs.into_iter().nth(prevout.index as usize))
	}

	fn is_spent(&self, prevout: &OutPoint) -> bool {
		self.transaction_meta(&prevout.hash)
			.and_then(|meta| meta.is_spent(prevout.index as usize))
			.unwrap_or(false)
	}
}

impl<T> BlockChain for BlockChainDatabase<T>
where
	T: DbInterface,
{
	fn insert(&self, block: IndexedBlock) -> Result<(), storage::Error> {
		BlockChainDatabase::insert(self, block)
	}

	fn rollback_best(&self) -> Result<SHA256D, storage::Error> {
		BlockChainDatabase::rollback_best(self)
	}

	fn canonize(&self, block_hash: &SHA256D) -> Result<(), storage::Error> {
		BlockChainDatabase::canonize(self, block_hash)
	}

	fn decanonize(&self) -> Result<SHA256D, storage::Error> {
		BlockChainDatabase::decanonize(self)
	}

	fn block_origin(&self, header: &IndexedBlockHeader) -> Result<BlockOrigin, storage::Error> {
		BlockChainDatabase::block_origin(self, header)
	}
}

#[cfg(test)]
mod tests {
	use super::SHA256D;
	use crate::blockchain_db::BlockChainDatabase;
	use crate::ham_db::HamDb;
	use chain::IndexedBlock;
	use storage::{AsSubstore, BlockMeta, BlockProvider, BlockRef, TransactionMetaProvider};
	use test_data::{block_h0, block_h1, block_h2};

	#[test]
	fn test_persistence() {
		std::fs::remove_file("bitcoin-rs.0.bc").unwrap();
		std::fs::remove_file("bitcoin-rs.0.lg").unwrap();
		std::fs::remove_file("bitcoin-rs.0.tb").unwrap();
		std::fs::remove_file("bitcoin-rs.0.bl").unwrap();

		let b0: IndexedBlock = block_h0().into();
		{
			let db = BlockChainDatabase::persistent().unwrap();
			db.insert(b0.clone()).unwrap();
			db.canonize(b0.hash()).unwrap();
		}
		{
			let db = BlockChainDatabase::persistent().unwrap();
			let block = db.block(BlockRef::Hash(b0.hash().clone())).unwrap();
			assert_eq!(block, b0);
		}
	}

	#[test]
	fn test_block_provider() {
		let db = BlockChainDatabase::transient().unwrap();

		let b0: IndexedBlock = block_h0().into();
		db.insert(b0.clone()).unwrap();
		db.canonize(b0.hash()).unwrap();

		let block_ref = BlockRef::Hash(b0.hash().clone());

		let block = db.block(block_ref).unwrap();
		assert_eq!(block, b0);

		let meta = db.as_block_provider().block_meta(block_ref).unwrap();
		assert_eq!(
			meta,
			BlockMeta {
				number: 0,
				n_tx: 1,
				n_chain_tx: 1
			}
		);

		let header = db.as_block_provider().block_header(block_ref).unwrap();
		assert_eq!(header, b0.header);

		let number = db.as_block_provider().block_number(b0.hash()).unwrap();
		assert_eq!(number, 0);

		let hash = db.as_block_provider().block_hash(0).unwrap();
		assert_eq!(&hash, b0.hash());

		assert!(db.as_block_provider().contains_block(block_ref));

		let txs = db.as_block_provider().block_transactions(block_ref);
		assert_eq!(txs.len(), 1);
		assert_eq!(txs.get(0), b0.transactions.get(0));

		let tx_hashes = db.as_block_provider().block_transaction_hashes(block_ref);
		assert_eq!(tx_hashes.len(), 1);
		assert_eq!(tx_hashes.get(0).unwrap(), &b0.transactions.get(0).unwrap().hash);
	}

	#[test]
	pub fn test_canonize() {
		let db = BlockChainDatabase::transient().unwrap();

		let b0: IndexedBlock = block_h0().into();
		db.insert(b0.clone()).unwrap();
		db.canonize(b0.hash()).unwrap();

		assert_best(&db, 0, b0.header.hash);

		let b1: IndexedBlock = block_h1().into();
		db.insert(b1.clone()).unwrap();
		db.canonize(b1.hash()).unwrap();

		assert_best(&db, 1, b1.header.hash);

		let b2: IndexedBlock = block_h2().into();
		db.insert(b2.clone()).unwrap();
		db.canonize(b2.hash()).unwrap();

		assert_best(&db, 2, b2.header.hash);

		let s2 = db.block(BlockRef::Hash(b2.hash().clone())).unwrap();
		s2.transactions.get(0).unwrap().raw.is_coinbase();

		assert_eq!(db.block_hash(0).unwrap(), b0.header.hash);
		assert_eq!(db.block_hash(1).unwrap(), b1.header.hash);
		assert_eq!(db.block_hash(2).unwrap(), b2.header.hash);
	}

	fn assert_best(db: &BlockChainDatabase<HamDb>, number: u32, hash: SHA256D) {
		let best = db.best_block();
		assert_eq!(best.number, number);
		assert_eq!(best.hash, hash);
	}

	#[test]
	fn test_block_not_found() {
		let db = BlockChainDatabase::transient().unwrap();

		assert!(db.block(BlockRef::Hash(block_h0().hash())).is_none());
		assert!(db.block(BlockRef::Number(5)).is_none());
		assert!(db.block_meta(BlockRef::Hash(block_h0().hash())).is_none());
		assert!(db.transaction_meta(&SHA256D::default()).is_none())
	}

	#[test]
	fn test_transaction_provider() {
		let db = BlockChainDatabase::transient().unwrap();

		let b0: IndexedBlock = block_h0().into();
		db.insert(b0.clone()).unwrap();
		db.canonize(b0.hash()).unwrap();
		let tx = db
			.as_transaction_meta_provider()
			.transaction_meta(&b0.transactions.get(0).unwrap().hash)
			.unwrap();

		assert_eq!(tx.height(), 0);
		assert!(tx.is_coinbase());
	}
}
