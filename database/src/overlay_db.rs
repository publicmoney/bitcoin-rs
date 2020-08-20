use crate::db_interface::DbInterface;
use bitcrypto::SHA256D;
use chain::{IndexedBlock, IndexedBlockHeader, IndexedTransaction};
use parking_lot::RwLock;
use std::collections::HashMap;
use storage::{BlockHeight, BlockMeta, Error, TransactionMeta};

#[derive(Default)]
struct MemoryDatabase {
	block_headers: HashMap<SHA256D, IndexedBlockHeader>,
	block_metas: HashMap<SHA256D, BlockMeta>,
	block_txs: HashMap<SHA256D, Vec<SHA256D>>,
	txs: HashMap<SHA256D, IndexedTransaction>,
	tx_metas: HashMap<SHA256D, TransactionMeta>,
	block_numbers: HashMap<u32, SHA256D>,
	best_block: Option<SHA256D>,
}

pub struct OverlayDatabase<'a, H>
where
	H: 'a + DbInterface,
{
	ham_db: &'a H,
	mem_db: RwLock<MemoryDatabase>,
}

impl<'a, T: DbInterface> OverlayDatabase<'a, T> {
	pub fn new(hammersbald: &'a T) -> Self {
		OverlayDatabase {
			ham_db: hammersbald,
			mem_db: RwLock::new(MemoryDatabase::default()),
		}
	}
}

impl<'a, T: DbInterface> DbInterface for OverlayDatabase<'a, T> {
	fn insert_block(&self, block: IndexedBlock) -> Result<(), storage::Error> {
		let mut db = self.mem_db.write();

		db.block_metas.insert(block.header.hash, BlockMeta::default());

		let mut tx_hashes = Vec::new();

		for tx in &block.transactions {
			tx_hashes.push(tx.hash);
			db.txs.insert(tx.hash, tx.clone());
			db.tx_metas.insert(tx.hash, TransactionMeta::new(0, tx.raw.outputs.len()));
		}

		db.block_txs.insert(block.header.hash, tx_hashes);
		db.block_headers.insert(block.header.hash, block.header);
		Ok(())
	}

	fn fetch_block(&self, block_hash: &SHA256D) -> Result<Option<IndexedBlock>, Error> {
		Ok(self
			.mem_db
			.read()
			.block_headers
			.get(block_hash)
			.cloned()
			.and_then(|header| {
				self.mem_db.read().block_txs.get(block_hash).and_then(|tx_hashes| {
					let txs = tx_hashes
						.iter()
						.map(|tx_hash| self.mem_db.read().txs.get(tx_hash).unwrap().clone())
						.collect::<Vec<IndexedTransaction>>();
					Some(IndexedBlock::new(header, txs))
				})
			})
			.or(self.ham_db.fetch_block(block_hash)?))
	}

	fn fetch_block_header(&self, block_hash: &SHA256D) -> Result<Option<IndexedBlockHeader>, storage::Error> {
		Ok(self
			.mem_db
			.read()
			.block_headers
			.get(block_hash)
			.cloned()
			.or(self.ham_db.fetch_block_header(block_hash)?))
	}

	fn fetch_block_meta(&self, block_hash: &SHA256D) -> Result<Option<BlockMeta>, storage::Error> {
		Ok(self
			.mem_db
			.read()
			.block_metas
			.get(block_hash)
			.cloned()
			.or(self.ham_db.fetch_block_meta(block_hash)?))
	}

	fn fetch_transactions(&self, block_hash: &SHA256D) -> Result<Option<Vec<IndexedTransaction>>, storage::Error> {
		Ok(self
			.mem_db
			.read()
			.block_txs
			.get(block_hash)
			.and_then(|tx_hashes| {
				Some(
					tx_hashes
						.iter()
						.map(|tx_hash| self.mem_db.read().txs.get(tx_hash).unwrap().clone())
						.collect::<Vec<IndexedTransaction>>(),
				)
			})
			.or(self.ham_db.fetch_transactions(block_hash)?))
	}

	fn fetch_transaction_hashes(&self, block_hash: &SHA256D) -> Result<Option<Vec<SHA256D>>, storage::Error> {
		Ok(self
			.mem_db
			.read()
			.block_txs
			.get(block_hash)
			.cloned()
			.or(self.ham_db.fetch_transaction_hashes(block_hash)?))
	}

	fn fetch_transaction(&self, tx_hash: &SHA256D) -> Result<Option<IndexedTransaction>, storage::Error> {
		Ok(self
			.mem_db
			.read()
			.txs
			.get(tx_hash)
			.cloned()
			.or(self.ham_db.fetch_transaction(tx_hash)?))
	}

	fn fetch_transaction_meta(&self, tx_hash: &SHA256D) -> Result<Option<TransactionMeta>, storage::Error> {
		Ok(self
			.mem_db
			.read()
			.tx_metas
			.get(tx_hash)
			.cloned()
			.or(self.ham_db.fetch_transaction_meta(tx_hash)?))
	}

	fn update_block_meta(&self, block_hash: &SHA256D, block_meta: &BlockMeta) -> Result<(), storage::Error> {
		self.mem_db.write().block_metas.insert(*block_hash, block_meta.clone());
		Ok(())
	}

	fn update_transaction_meta(&self, tx_hash: &SHA256D, tx_meta: TransactionMeta) -> Result<(), storage::Error> {
		self.mem_db.write().tx_metas.insert(*tx_hash, tx_meta.clone());
		Ok(())
	}

	fn set_block_by_number(&self, block_hash: &SHA256D, number: u32) -> Result<(), storage::Error> {
		self.mem_db.write().block_numbers.insert(number, *block_hash);
		Ok(())
	}

	fn block_hash(&self, number: u32) -> Result<Option<SHA256D>, storage::Error> {
		Ok(self
			.mem_db
			.read()
			.block_numbers
			.get(&number)
			.cloned()
			.or(self.ham_db.block_hash(number)?))
	}

	fn best_block(&self) -> Result<BlockHeight, storage::Error> {
		Ok(self
			.mem_db
			.read()
			.best_block
			.and_then(|hash| {
				self.mem_db
					.read()
					.block_metas
					.get(&hash)
					.and_then(|meta| Some(BlockHeight { number: meta.number, hash }))
			})
			.or(Some(self.ham_db.best_block()?))
			.unwrap_or_default())
	}

	fn set_best(&self, block_hash: &SHA256D) -> Result<(), storage::Error> {
		self.mem_db.write().best_block = Some(*block_hash);
		Ok(())
	}

	fn flush(&self) -> Result<(), storage::Error> {
		let mut blocks = vec![];
		for header in self.mem_db.read().block_headers.values() {
			let block = self.fetch_block(&header.hash)?.unwrap();
			blocks.push(block);
		}
		let mut db = self.mem_db.write();
		for block in blocks {
			self.ham_db.insert_block(block)?;
		}
		for (hash, block_meta) in db.block_metas.iter() {
			self.ham_db.update_block_meta(hash, block_meta)?;
		}
		for (hash, tx_meta) in db.tx_metas.iter() {
			self.ham_db.update_transaction_meta(hash, tx_meta.clone())?;
		}
		for (number, hash) in db.block_numbers.iter() {
			self.ham_db.set_block_by_number(hash, *number)?;
		}
		if db.best_block.is_some() {
			self.ham_db.set_best(&db.best_block.unwrap())?;
		}

		*db = MemoryDatabase::default();
		self.ham_db.flush()
	}

	fn shutdown(&self) -> Result<(), Error> {
		unreachable!()
	}
}
