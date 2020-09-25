use crate::db_interface::DbInterface;
use crate::ham_types::{DbBlock, DbInputKey, DbOutputKey, DbTransaction};
use bitcrypto::SHA256D;
use chain::{BlockHeader, IndexedBlock, IndexedBlockHeader, IndexedTransaction, Transaction};
use hammersbald::{persistent, transient, HammersbaldAPI};
use parking_lot::RwLock;
use serialization::{deserialize, serialize, Deserializable, Serializable};
use std::sync::Arc;
use storage::{BlockHeight, BlockMeta, Error, TransactionMeta};

pub type PRef = u64;
const BEST_PREF: PRef = 0;

pub struct HamDb {
	hammersbald: Arc<RwLock<Box<dyn HammersbaldAPI>>>,
}

impl HamDb {
	pub fn transient() -> Result<HamDb, storage::Error> {
		Ok(Self::new(transient().map_err(from_ham)?))
	}

	pub fn persistent(db_path: &str, db_name: &str, db_cache_size_mb: usize) -> Result<HamDb, storage::Error> {
		Ok(Self::new(persistent(db_path, db_name, db_cache_size_mb).map_err(from_ham)?))
	}

	fn new(hammersbald: Box<dyn HammersbaldAPI>) -> HamDb {
		HamDb {
			hammersbald: Arc::new(RwLock::new(hammersbald)),
		}
	}

	fn get_by_pref<T>(&self, pref: PRef) -> Result<Option<T>, storage::Error>
	where
		T: Deserializable,
	{
		let (_, v) = self.hammersbald.read().get(pref).map_err(from_ham)?;
		let result = deserialize::<&[u8], T>(&v).map_err(from_serial)?;
		return Ok(Some(result));
	}

	fn get_by_key<K, T>(&self, key: &K) -> Result<Option<(PRef, T)>, storage::Error>
	where
		K: Serializable,
		T: Deserializable,
	{
		if let Some((pref, v)) = self.hammersbald.read().get_keyed(&serialize(key)).map_err(from_ham)? {
			let result = deserialize::<&[u8], T>(&v).map_err(from_serial)?;
			return Ok(Some((pref, result)));
		}
		Ok(None)
	}

	fn get_keyed<K, T>(&self, pref: PRef) -> Result<Option<(K, T)>, storage::Error>
	where
		K: Deserializable,
		T: Deserializable,
	{
		let (k, v) = self.hammersbald.read().get(pref).map_err(from_ham)?;
		let key = deserialize::<&[u8], K>(&k).map_err(from_serial)?;
		let result = deserialize::<&[u8], T>(&v).map_err(from_serial)?;
		Ok(Some((key, result)))
	}

	fn get_key<K>(&self, pref: PRef) -> Result<K, storage::Error>
	where
		K: Deserializable,
	{
		deserialize::<&[u8], K>(&self.hammersbald.read().get(pref).map_err(from_ham)?.0).map_err(from_serial)
	}

	fn put<T>(&self, data: &T) -> Result<PRef, storage::Error>
	where
		T: Serializable,
	{
		self.hammersbald.write().put(&serialize(data)).map_err(from_ham)
	}

	fn put_keyed<K, T>(&self, key: &K, data: &T) -> Result<PRef, storage::Error>
	where
		K: Serializable,
		T: Serializable,
	{
		self.hammersbald
			.write()
			.put_keyed(&serialize(key), &serialize(data))
			.map_err(from_ham)
	}

	fn set<T>(&self, pref: PRef, data: &T) -> Result<PRef, storage::Error>
	where
		T: Serializable,
	{
		self.hammersbald.write().set(pref, &serialize(data)).map_err(from_ham)
	}
}

impl DbInterface for HamDb {
	fn insert_block(&self, block: IndexedBlock) -> Result<(), storage::Error> {
		debug!("Inserting block header: {} {:?}", block.header.hash, block.header.raw);
		let header_pref = self.put(&block.header.raw)?;

		let mut tx_refs: Vec<PRef> = vec![];

		for tx in &block.transactions {
			let tx_meta = TransactionMeta::new(0, tx.raw.outputs.len());
			debug!("Inserting transaction meta: {} {:?}", tx.hash, tx_meta);
			let meta_pref = self.put(&tx_meta)?;
			debug!("Inserting transaction: {} {:?}", tx.hash, tx.raw);
			let tx_pref = self.put(&tx.raw)?;
			let db_tx = DbTransaction { tx_pref, meta_pref };
			let db_tx_pref = self.put_keyed(&tx.hash, &db_tx)?;

			for input in &tx.raw.inputs {
				let input = DbInputKey::from(input);
				self.put_keyed(&input, &db_tx_pref)?;
			}
			for output in &tx.raw.outputs {
				let key = DbOutputKey::from(output);
				self.put_keyed(&key, &db_tx_pref)?;
			}
			tx_refs.push(db_tx_pref);
		}
		let mut meta = BlockMeta::default();
		meta.n_tx = block.transactions.len() as u32;

		let meta_pref = self.put(&meta)?;

		let db_block = DbBlock {
			header: header_pref,
			txs: tx_refs,
			meta: meta_pref,
		};

		debug!("Inserting db_block: {} {:?}", block.header.hash, db_block);
		self.put_keyed(&block.header.hash, &db_block)?;
		Ok(())
	}

	fn fetch_block(&self, block_hash: &SHA256D) -> Result<Option<IndexedBlock>, storage::Error> {
		let header = self.fetch_block_header(block_hash)?;
		if let Some(header) = header {
			let txs = self.fetch_transactions(block_hash)?;
			return Ok(txs.and_then(|transactions| Some(IndexedBlock::new(header, transactions))));
		}
		Ok(None)
	}

	fn fetch_block_header(&self, block_hash: &SHA256D) -> Result<Option<IndexedBlockHeader>, storage::Error> {
		if let Some((_, db_block)) = self.get_by_key::<SHA256D, DbBlock>(block_hash)? {
			return Ok(self.get_by_pref::<BlockHeader>(db_block.header)?.map(|header| header.into()));
		}
		Ok(None)
	}

	fn fetch_block_meta(&self, block_hash: &SHA256D) -> Result<Option<BlockMeta>, storage::Error> {
		if let Some((_, db_block)) = self.get_by_key::<SHA256D, DbBlock>(block_hash)? {
			return self.get_by_pref::<BlockMeta>(db_block.meta);
		}
		Ok(None)
	}

	fn fetch_transactions(&self, block_hash: &SHA256D) -> Result<Option<Vec<IndexedTransaction>>, storage::Error> {
		if let Some((_, db_block)) = self.get_by_key::<SHA256D, DbBlock>(block_hash)? {
			let mut txs = Vec::new();
			for tx_ref in &db_block.txs {
				if let Some(db_tx) = self.get_by_pref::<DbTransaction>(*tx_ref)? {
					if let Some(tx) = self.get_by_pref::<Transaction>(db_tx.tx_pref)? {
						txs.push(tx.into());
					}
				}
			}
			return if txs.len() == db_block.txs.len() {
				Ok(Some(txs))
			} else {
				Err(storage::Error::InconsistentData)
			};
		}
		Ok(None)
	}

	fn fetch_transaction_hashes(&self, block_hash: &SHA256D) -> Result<Option<Vec<SHA256D>>, storage::Error> {
		if let Some((_, db_block)) = self.get_by_key::<SHA256D, DbBlock>(block_hash)? {
			let mut tx_hashes = Vec::new();
			for tx_ref in &db_block.txs {
				if let Some((k, _)) = self.get_keyed::<SHA256D, DbTransaction>(*tx_ref)? {
					tx_hashes.push(k);
					continue;
				}
				return Err(storage::Error::InconsistentData);
			}
			return Ok(Some(tx_hashes));
		}
		Ok(None)
	}

	fn fetch_transaction(&self, tx_hash: &SHA256D) -> Result<Option<IndexedTransaction>, storage::Error> {
		if let Some((_, db_tx)) = self.get_by_key::<SHA256D, DbTransaction>(tx_hash)? {
			if let Some(tx) = self.get_by_pref::<Transaction>(db_tx.tx_pref)? {
				return Ok(Some(tx.into()));
			}
		}
		Ok(None)
	}

	fn fetch_transaction_meta(&self, tx_hash: &SHA256D) -> Result<Option<TransactionMeta>, storage::Error> {
		if let Some((_, db_tx)) = self.get_by_key::<SHA256D, DbTransaction>(tx_hash)? {
			return self.get_by_pref::<TransactionMeta>(db_tx.meta_pref);
		}
		Ok(None)
	}

	fn update_block_meta(&self, block_hash: &SHA256D, block_meta: &BlockMeta) -> Result<(), storage::Error> {
		debug!("Update block meta: {} {:?}", block_hash, block_meta);
		if let Some((_, db_block)) = self.get_by_key::<SHA256D, DbBlock>(block_hash)? {
			self.set(db_block.meta, block_meta)?;
			return Ok(());
		}
		Err(storage::Error::InconsistentData)
	}

	fn update_transaction_meta(&self, tx_hash: &SHA256D, tx_meta: TransactionMeta) -> Result<(), storage::Error> {
		debug!("Update tx meta: {} {:?}", tx_hash, tx_meta);
		if let Some((_, db_tx)) = self.get_by_key::<SHA256D, DbTransaction>(tx_hash)? {
			self.set(db_tx.meta_pref, &tx_meta)?;
			return Ok(());
		}
		Err(storage::Error::InconsistentData)
	}

	fn set_block_by_number(&self, block_hash: &SHA256D, number: u32) -> Result<(), storage::Error> {
		debug!("Set block number: {} {:?}", block_hash, number);
		if let Some((block_pref, _)) = self.get_by_key::<SHA256D, DbBlock>(block_hash)? {
			match self.get_by_key::<u32, PRef>(&number)? {
				Some((pref, _)) => self.set(pref, &block_pref)?,
				None => self.put_keyed(&number, &block_pref)?,
			};
			return Ok(());
		}
		Err(storage::Error::InconsistentData)
	}

	fn block_hash(&self, number: u32) -> Result<Option<SHA256D>, storage::Error> {
		if let Some((_, pref)) = self.get_by_key::<u32, PRef>(&number)? {
			return Ok(Some(self.get_key(pref)?));
		}
		Ok(None)
	}

	fn best_block(&self) -> Result<BlockHeight, storage::Error> {
		match self.get_by_pref::<u32>(BEST_PREF) {
			Ok(Some(best_number)) => {
				for i in 0..100 {
					let number = best_number - i;
					if let Some((_, block_pref)) = self.get_by_key::<u32, PRef>(&number)? {
						if let Some((hash, _db_block)) = self.get_keyed::<SHA256D, DbBlock>(block_pref)? {
							return Ok(BlockHeight { hash, number });
						}
					}
				}
				return Err(storage::Error::DatabaseError("Failed to fetch best block".to_string()));
			}
			_ => {
				let pref = self.put(&0)?;
				if pref != BEST_PREF {
					return Err(storage::Error::DatabaseError("Database initialisation error".to_string()));
				}
				self.flush()?;
				Ok(BlockHeight::default())
			}
		}
	}

	fn set_best(&self, block_number: u32) -> Result<(), storage::Error> {
		self.set(BEST_PREF, &block_number)?;
		return Ok(());
	}

	fn flush(&self) -> Result<(), storage::Error> {
		self.hammersbald.write().batch().map_err(from_ham)
	}

	fn stats(&self) -> Result<(), Error> {
		self.hammersbald.read().stats();
		Ok(())
	}

	fn size(&self) -> u64 {
		self.hammersbald.read().size()
	}

	fn shutdown(&self) -> Result<(), storage::Error> {
		self.hammersbald.write().shutdown().map_err(from_ham)
	}
}

fn from_ham(e: hammersbald::Error) -> storage::Error {
	storage::Error::DatabaseError(e.to_string())
}

fn from_serial(e: serialization::Error) -> storage::Error {
	storage::Error::DatabaseError(e.to_string())
}

impl Drop for HamDb {
	fn drop(&mut self) {
		self.shutdown().unwrap_or_default();
	}
}

impl Clone for HamDb {
	fn clone(&self) -> Self {
		HamDb {
			hammersbald: self.hammersbald.clone(),
		}
	}
}
