use crate::bytes::Bytes;
use crate::kv::{
	AutoFlushingOverlayDatabase, CacheDatabase, DatabaseConfig, DiskDatabase, Key, KeyState, KeyValue, KeyValueDatabase, MemoryDatabase,
	OverlayDatabase, Transaction as DBTransaction, Value, COL_BLOCK_META,
};
use crate::kv::{COL_BLOCK_HASHES, COL_BLOCK_HEADERS, COL_BLOCK_TRANSACTIONS, COL_COUNT, COL_TRANSACTIONS, COL_TRANSACTIONS_META};
use bitcrypto::SHA256D;
use chain::{IndexedBlock, IndexedBlockHeader, IndexedTransaction, OutPoint, TransactionOutput};
use parking_lot::RwLock;
use ser::{deserialize, serialize, List};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use storage::{
	BlockChain, BlockHeaderProvider, BlockHeight, BlockMeta, BlockOrigin, BlockProvider, BlockRef, CanonStore, ConfigStore, Error,
	ForkChain, Forkable, SideChainOrigin, Store, TransactionMeta, TransactionMetaProvider, TransactionOutputProvider, TransactionProvider,
};

// TODO Don't need to store best block and best hash, should just use hash and BlockMeta
const KEY_BEST_BLOCK_NUMBER: &'static str = "best_block_number";
const KEY_BEST_BLOCK_HASH: &'static str = "best_block_hash";

const MAX_FORK_ROUTE_PRESET: usize = 2048;

pub struct BlockChainDatabase<T>
where
	T: KeyValueDatabase,
{
	best_block: RwLock<BlockHeight>,
	db: T,
}

pub struct ForkChainDatabase<'a, T>
where
	T: 'a + KeyValueDatabase,
{
	blockchain: BlockChainDatabase<OverlayDatabase<'a, T>>,
}

impl<'a, T> ForkChain for ForkChainDatabase<'a, T>
where
	T: KeyValueDatabase,
{
	fn store(&self) -> &dyn Store {
		&self.blockchain
	}

	fn flush(&self) -> Result<(), Error> {
		self.blockchain.db.flush().map_err(Error::DatabaseError)
	}
}

impl BlockChainDatabase<CacheDatabase<AutoFlushingOverlayDatabase<DiskDatabase>>> {
	pub fn open_at_path<P>(path: P, total_cache: usize) -> Result<Self, Error>
	where
		P: AsRef<Path>,
	{
		fs::create_dir_all(path.as_ref()).map_err(|err| Error::DatabaseError(err.to_string()))?;
		let mut cfg = DatabaseConfig::with_columns(Some(COL_COUNT));

		cfg.set_cache(Some(COL_TRANSACTIONS), total_cache / 4);
		cfg.set_cache(Some(COL_TRANSACTIONS_META), total_cache / 4);
		cfg.set_cache(Some(COL_BLOCK_HEADERS), total_cache / 4);

		cfg.set_cache(Some(COL_BLOCK_HASHES), total_cache / 12);
		cfg.set_cache(Some(COL_BLOCK_TRANSACTIONS), total_cache / 12);
		cfg.set_cache(Some(COL_BLOCK_META), total_cache / 12);

		cfg.bloom_filters.insert(Some(COL_TRANSACTIONS_META), 32);

		match DiskDatabase::open(cfg, path) {
			Ok(db) => Ok(Self::open_with_cache(db)),
			Err(err) => Err(Error::DatabaseError(err)),
		}
	}
}

impl BlockChainDatabase<MemoryDatabase> {
	pub fn init_test_chain(blocks: Vec<IndexedBlock>) -> Self {
		let store = BlockChainDatabase::open(MemoryDatabase::default());

		for block in blocks {
			let hash = block.hash().clone();
			store.insert(block).unwrap();
			store.canonize(&hash).unwrap();
		}
		store
	}
}

impl<T> BlockChainDatabase<CacheDatabase<AutoFlushingOverlayDatabase<T>>>
where
	T: KeyValueDatabase,
{
	pub fn open_with_cache(db: T) -> Self {
		let db = CacheDatabase::new(AutoFlushingOverlayDatabase::new(db, 50));
		let best_block = Self::read_best_block(&db).unwrap_or_default();
		BlockChainDatabase {
			best_block: RwLock::new(best_block),
			db,
		}
	}
}

impl<T> BlockChainDatabase<T>
where
	T: KeyValueDatabase,
{
	fn read_best_block(db: &T) -> Option<BlockHeight> {
		let best_number = db
			.get(&Key::Meta(KEY_BEST_BLOCK_NUMBER))
			.map(KeyState::into_option)
			.map(|x| x.and_then(Value::as_meta));
		let best_hash = db
			.get(&Key::Meta(KEY_BEST_BLOCK_HASH))
			.map(KeyState::into_option)
			.map(|x| x.and_then(Value::as_meta));

		match (best_number, best_hash) {
			(Ok(None), Ok(None)) => None,
			(Ok(Some(number)), Ok(Some(hash))) => {
				let number: u32 = deserialize(&**number).expect("Inconsistent DB. Invalid best block number.");
				let hash = deserialize(&**hash).expect("Inconsistent DB. Invalid best block hash.");
				Some(BlockHeight { hash, number })
			}
			_ => panic!("Inconsistent DB"),
		}
	}

	pub fn open(db: T) -> Self {
		let best_block = Self::read_best_block(&db).unwrap_or_default();
		BlockChainDatabase {
			best_block: RwLock::new(best_block),
			db,
		}
	}

	pub fn best_block(&self) -> BlockHeight {
		self.best_block.read().clone()
	}

	pub fn fork(&self, side_chain: SideChainOrigin) -> Result<ForkChainDatabase<T>, Error> {
		let overlay = BlockChainDatabase::open(OverlayDatabase::new(&self.db));

		for hash in side_chain.decanonized_route.into_iter().rev() {
			let decanonized_hash = overlay.decanonize()?;
			assert_eq!(hash, decanonized_hash);
		}

		for block_hash in &side_chain.canonized_route {
			overlay.canonize(block_hash)?;
		}

		let fork = ForkChainDatabase { blockchain: overlay };

		Ok(fork)
	}

	pub fn switch_to_fork(&self, fork: ForkChainDatabase<T>) -> Result<(), Error> {
		let mut best_block = self.best_block.write();
		*best_block = fork.blockchain.best_block.read().clone();
		fork.blockchain.db.flush().map_err(Error::DatabaseError)
	}

	pub fn block_origin(&self, header: &IndexedBlockHeader) -> Result<BlockOrigin, Error> {
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
			return Err(Error::UnknownParent);
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
					sidechain_route.push(next_hash.clone());
					next_hash = self
						.block_header(next_hash.into())
						.expect("not to find orphaned side chain in database; qed")
						.raw
						.previous_header_hash;
				}
			}
		}

		Err(Error::AncientFork)
	}

	pub fn insert(&self, block: IndexedBlock) -> Result<(), Error> {
		if self.contains_block(block.hash().clone().into()) {
			return Ok(());
		}

		let parent_hash = block.header.raw.previous_header_hash.clone();
		if parent_hash != SHA256D::default() && !self.contains_block(parent_hash.clone().into()) {
			return Err(Error::UnknownParent);
		}

		let mut update = DBTransaction::new();
		let tx_hashes = block.transactions.iter().map(|tx| tx.hash.clone()).collect::<Vec<_>>();
		update.insert(KeyValue::BlockHeader(*block.hash(), block.header.raw));
		update.insert(KeyValue::BlockTransactions(block.header.hash.clone(), List::from(tx_hashes)));

		for tx in block.transactions.into_iter() {
			update.insert(KeyValue::Transaction(tx.hash, tx.raw));
		}

		self.db.write(update).map_err(Error::DatabaseError)
	}

	/// Rollbacks single best block
	fn rollback_best(&self) -> Result<SHA256D, Error> {
		let best_block = self.best_block.read().clone();
		let tx_to_decanonize = self.block_transaction_hashes(best_block.hash.into());
		let decanonized_hash = self.decanonize()?;
		debug_assert_eq!(best_block.hash, decanonized_hash);

		// and now remove decanonized block from database
		// all code currently works in assumption that origin of all blocks is one of:
		// {CanonChain, SideChain, SideChainBecomingCanonChain}
		let mut update = DBTransaction::new();
		update.delete(Key::BlockHeader(decanonized_hash.clone()));
		update.delete(Key::BlockTransactions(decanonized_hash.clone()));
		for tx_hash in tx_to_decanonize {
			update.delete(Key::Transaction(tx_hash));
		}

		self.db.write(update).map_err(Error::DatabaseError)?;

		Ok(self.best_block.read().hash)
	}

	/// Marks block as a new best block.
	/// Block must be already inserted into db, and it's parent must be current best block.
	/// Updates meta data.
	pub fn canonize(&self, hash: &SHA256D) -> Result<(), Error> {
		let mut best_block = self.best_block.write();
		let block = match self.block(hash.clone().into()) {
			Some(block) => block,
			None => {
				error!(target: "db", "Block is not found during canonization: {}", hash);
				return Err(Error::CannotCanonize);
			}
		};

		if best_block.hash != block.header.raw.previous_header_hash {
			error!(
				target: "db",
				"Wrong best block during canonization. Best {}, parent: {}",
				best_block.hash,
				block.header.raw.previous_header_hash,
			);
			return Err(Error::CannotCanonize);
		}

		let new_meta = if block.header.raw.previous_header_hash == SHA256D::default() {
			assert_eq!(best_block.number, 0);
			BlockMeta {
				number: 0,
				n_tx: block.transactions.len() as u32,
				n_chain_tx: block.transactions.len() as u32,
			}
		} else {
			let best_meta = self.block_meta(best_block.hash.into()).ok_or(Error::UnknownParent)?;
			BlockMeta {
				number: best_block.number + 1,
				n_tx: block.transactions.len() as u32,
				n_chain_tx: best_meta.n_chain_tx + block.transactions.len() as u32,
			}
		};

		let new_best_block = BlockHeight {
			hash: *hash,
			number: new_meta.number,
		};

		trace!(target: "db", "canonize {:?}", new_best_block);

		let mut update = DBTransaction::new();
		update.insert(KeyValue::BlockHash(new_best_block.number, new_best_block.hash));
		update.insert(KeyValue::BlockMeta(*block.hash(), new_meta));
		update.insert(KeyValue::Meta(KEY_BEST_BLOCK_HASH, serialize(&new_best_block.hash)));
		update.insert(KeyValue::Meta(KEY_BEST_BLOCK_NUMBER, serialize(&new_best_block.number)));

		let mut modified_meta: HashMap<SHA256D, TransactionMeta> = HashMap::new();
		if let Some(tx) = block.transactions.first() {
			let meta = TransactionMeta::new_coinbase(new_best_block.number, tx.raw.outputs.len());
			modified_meta.insert(tx.hash.clone(), meta);
		}

		for tx in block.transactions.iter().skip(1) {
			modified_meta.insert(tx.hash.clone(), TransactionMeta::new(new_best_block.number, tx.raw.outputs.len()));

			for input in &tx.raw.inputs {
				use std::collections::hash_map::Entry;

				match modified_meta.entry(input.previous_output.hash.clone()) {
					Entry::Occupied(mut entry) => {
						let meta = entry.get_mut();
						meta.denote_used(input.previous_output.index as usize);
					}
					Entry::Vacant(entry) => {
						let mut meta = self.transaction_meta(&input.previous_output.hash).ok_or_else(|| {
							error!(
								target: "db",
								"Cannot find tx meta during canonization of tx {}: {}/{}",
								tx.hash,
								input.previous_output.hash,
								input.previous_output.index,
							);
							Error::CannotCanonize
						})?;
						meta.denote_used(input.previous_output.index as usize);
						entry.insert(meta);
					}
				}
			}
		}

		for (hash, meta) in modified_meta.into_iter() {
			update.insert(KeyValue::TransactionMeta(hash, meta));
		}

		self.db.write(update).map_err(Error::DatabaseError)?;
		*best_block = new_best_block;
		Ok(())
	}

	pub fn decanonize(&self) -> Result<SHA256D, Error> {
		let mut best_block = self.best_block.write();
		let block = match self.block(best_block.hash.clone().into()) {
			Some(block) => block,
			None => {
				error!(target: "db", "Block is not found during decanonization: {}", best_block.hash);
				return Err(Error::CannotDecanonize);
			}
		};
		let block_number = best_block.number;
		let block_hash = best_block.hash.clone();

		let new_best_block = BlockHeight {
			hash: block.header.raw.previous_header_hash.clone(),
			number: if best_block.number > 0 {
				best_block.number - 1
			} else {
				assert!(block.header.raw.previous_header_hash == SHA256D::default());
				0
			},
		};

		trace!(target: "db", "decanonize, new best: {:?}", new_best_block);

		let mut update = DBTransaction::new();
		update.delete(Key::BlockHash(block_number));
		update.delete(Key::BlockMeta(block_hash));
		update.insert(KeyValue::Meta(KEY_BEST_BLOCK_HASH, serialize(&new_best_block.hash)));
		update.insert(KeyValue::Meta(KEY_BEST_BLOCK_NUMBER, serialize(&new_best_block.number)));

		let mut modified_meta: HashMap<SHA256D, TransactionMeta> = HashMap::new();
		for tx in block.transactions.iter().skip(1) {
			for input in &tx.raw.inputs {
				use std::collections::hash_map::Entry;

				match modified_meta.entry(input.previous_output.hash.clone()) {
					Entry::Occupied(mut entry) => {
						let meta = entry.get_mut();
						meta.denote_unused(input.previous_output.index as usize);
					}
					Entry::Vacant(entry) => {
						let mut meta = self.transaction_meta(&input.previous_output.hash).ok_or_else(|| {
							error!(
								target: "db",
								"Cannot find tx meta during decanonization of tx {}: {}/{}",
								tx.hash,
								input.previous_output.hash,
								input.previous_output.index,
							);
							Error::CannotDecanonize
						})?;
						meta.denote_unused(input.previous_output.index as usize);
						entry.insert(meta);
					}
				}
			}
		}

		for (hash, meta) in modified_meta {
			update.insert(KeyValue::TransactionMeta(hash, meta));
		}

		for tx in block.transactions {
			update.delete(Key::TransactionMeta(tx.hash));
		}

		self.db.write(update).map_err(Error::DatabaseError)?;
		*best_block = new_best_block;
		Ok(block_hash)
	}

	fn get(&self, key: Key) -> Option<Value> {
		self.db.get(&key).expect("db value to be fine").into_option()
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
	T: KeyValueDatabase,
{
	fn block_header_bytes(&self, block_ref: BlockRef) -> Option<Bytes> {
		self.block_header(block_ref).map(|header| serialize(&header.raw))
	}

	fn block_header(&self, block_ref: BlockRef) -> Option<IndexedBlockHeader> {
		self.resolve_hash(block_ref).and_then(|block_hash| {
			self.get(Key::BlockHeader(block_hash.clone()))
				.and_then(Value::as_block_header)
				.map(|header| IndexedBlockHeader::new(block_hash, header))
		})
	}
}

impl<T> BlockProvider for BlockChainDatabase<T>
where
	T: KeyValueDatabase,
{
	fn block_meta(&self, block_ref: BlockRef) -> Option<BlockMeta> {
		let hash = match block_ref {
			BlockRef::Number(n) => self.block_hash(n).unwrap(),
			BlockRef::Hash(h) => h,
		};
		self.get(Key::BlockMeta(hash)).and_then(Value::as_block_meta)
	}

	fn block_hash(&self, number: u32) -> Option<SHA256D> {
		self.get(Key::BlockHash(number)).and_then(Value::as_block_hash)
	}

	fn block_number(&self, hash: &SHA256D) -> Option<u32> {
		self.block_meta(BlockRef::Hash(*hash)).map(|b| b.number)
	}

	fn block(&self, block_ref: BlockRef) -> Option<IndexedBlock> {
		self.resolve_hash(block_ref).and_then(|block_hash| {
			self.block_header(block_hash.clone().into()).map(|header| {
				let transactions = self.block_transactions(block_hash.into());
				IndexedBlock::new(header, transactions)
			})
		})
	}

	fn contains_block(&self, block_ref: BlockRef) -> bool {
		self.resolve_hash(block_ref)
			.and_then(|hash| self.get(Key::BlockHeader(hash)))
			.is_some()
	}

	fn block_transaction_hashes(&self, block_ref: BlockRef) -> Vec<SHA256D> {
		self.resolve_hash(block_ref)
			.and_then(|hash| self.get(Key::BlockTransactions(hash)))
			.and_then(Value::as_block_transactions)
			.map(List::into)
			.unwrap_or_default()
	}

	fn block_transactions(&self, block_ref: BlockRef) -> Vec<IndexedTransaction> {
		self.block_transaction_hashes(block_ref)
			.into_iter()
			.filter_map(|hash| {
				self.get(Key::Transaction(hash))
					.and_then(Value::as_transaction)
					.map(|tx| IndexedTransaction::new(hash, tx))
			})
			.collect()
	}
}

impl<T> TransactionMetaProvider for BlockChainDatabase<T>
where
	T: KeyValueDatabase,
{
	fn transaction_meta(&self, hash: &SHA256D) -> Option<TransactionMeta> {
		self.get(Key::TransactionMeta(*hash)).and_then(Value::as_transaction_meta)
	}
}

impl<T> TransactionProvider for BlockChainDatabase<T>
where
	T: KeyValueDatabase,
{
	fn transaction_bytes(&self, hash: &SHA256D) -> Option<Bytes> {
		self.transaction(hash).map(|tx| serialize(&tx.raw))
	}

	fn transaction(&self, hash: &SHA256D) -> Option<IndexedTransaction> {
		self.get(Key::Transaction(*hash))
			.and_then(Value::as_transaction)
			.map(|tx| IndexedTransaction::new(*hash, tx))
	}
}

impl<T> TransactionOutputProvider for BlockChainDatabase<T>
where
	T: KeyValueDatabase,
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
	T: KeyValueDatabase,
{
	fn insert(&self, block: IndexedBlock) -> Result<(), Error> {
		BlockChainDatabase::insert(self, block)
	}

	fn rollback_best(&self) -> Result<SHA256D, Error> {
		BlockChainDatabase::rollback_best(self)
	}

	fn canonize(&self, block_hash: &SHA256D) -> Result<(), Error> {
		BlockChainDatabase::canonize(self, block_hash)
	}

	fn decanonize(&self) -> Result<SHA256D, Error> {
		BlockChainDatabase::decanonize(self)
	}

	fn block_origin(&self, header: &IndexedBlockHeader) -> Result<BlockOrigin, Error> {
		BlockChainDatabase::block_origin(self, header)
	}
}

impl<T> Forkable for BlockChainDatabase<T>
where
	T: KeyValueDatabase,
{
	fn fork<'a>(&'a self, side_chain: SideChainOrigin) -> Result<Box<dyn ForkChain + 'a>, Error> {
		BlockChainDatabase::fork(self, side_chain).map(|fork_chain| {
			let boxed: Box<dyn ForkChain> = Box::new(fork_chain);
			boxed
		})
	}

	fn switch_to_fork<'a>(&self, fork: Box<dyn ForkChain + 'a>) -> Result<(), Error> {
		let mut best_block = self.best_block.write();
		*best_block = fork.store().best_block();
		fork.flush()
	}
}

impl<T> CanonStore for BlockChainDatabase<T>
where
	T: KeyValueDatabase,
{
	fn as_store(&self) -> &dyn Store {
		&*self
	}
}

impl<T> Store for BlockChainDatabase<T>
where
	T: KeyValueDatabase,
{
	fn best_block(&self) -> BlockHeight {
		BlockChainDatabase::best_block(self)
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

impl<T> ConfigStore for BlockChainDatabase<T>
where
	T: KeyValueDatabase,
{
	fn consensus_fork(&self) -> Result<Option<String>, Error> {
		match self
			.db
			.get(&Key::Configuration("consensus_fork"))
			.map(KeyState::into_option)
			.map(|x| x.and_then(Value::as_configuration))
		{
			Ok(Some(consensus_fork)) => String::from_utf8(consensus_fork.into())
				.map_err(|e| Error::DatabaseError(format!("{}", e)))
				.map(Some),
			Ok(None) => Ok(None),
			Err(e) => Err(Error::DatabaseError(e.into())),
		}
	}

	fn set_consensus_fork(&self, consensus_fork: &str) -> Result<(), Error> {
		let mut update = DBTransaction::new();
		update.insert(KeyValue::Configuration("consensus_fork", consensus_fork.as_bytes().into()));
		self.db.write(update).map_err(Error::DatabaseError)
	}
}
