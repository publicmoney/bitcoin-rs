use crate::db_interface::DbInterface;
use crate::ham_types::{DbBlock, DbInputKey, DbOutput, DbTransaction};
use bitcrypto::{Hash, SHA256D};
use chain::{BlockHeader, IndexedBlock, IndexedBlockHeader, IndexedTransaction, Transaction};
use hammersbald::{persistent, transient, HammersbaldAPI, PRef};
use parking_lot::RwLock;
use serialization::{deserialize, serialize, Deserializable};
use std::path::Path;
use std::sync::Arc;
use storage::{BlockHeight, BlockMeta, TransactionMeta};

pub struct HamDb {
	hammersbald: Arc<RwLock<Box<dyn HammersbaldAPI>>>,
}

impl HamDb {
	pub fn transient() -> Result<HamDb, storage::Error> {
		Ok(Self::new(transient(1).map_err(from_ham)?))
	}

	pub fn persistent(db_path: String, db_cache_size_mb: usize) -> Result<HamDb, storage::Error> {
		if !Path::new(&db_path).exists() {
			std::fs::create_dir(&db_path).unwrap();
		}
		let full_path = format!("{}/bitcoin-rs", db_path);
		Ok(Self::new(persistent(&full_path, db_cache_size_mb, 1).map_err(from_ham)?))
	}

	fn new(hammersbald: Box<dyn HammersbaldAPI>) -> HamDb {
		HamDb {
			hammersbald: Arc::new(RwLock::new(hammersbald)),
		}
	}

	fn best_pref() -> PRef {
		PRef::from(0)
	}

	fn get_keyed_type<T>(&self, hash: &SHA256D) -> Result<Option<(PRef, T)>, storage::Error>
	where
		T: Deserializable,
	{
		if let Some((k, v)) = self.get_keyed(&serialize(hash))? {
			let result = deserialize::<&[u8], T>(&v).map_err(from_serial)?;
			return Ok(Some((k, result)));
		}
		Ok(None)
	}

	fn get_type<T>(&self, pref: PRef) -> Result<Option<T>, storage::Error>
	where
		T: Deserializable,
	{
		let (_, v) = self.get(pref)?;
		let result = deserialize::<&[u8], T>(&v).map_err(from_serial)?;
		return Ok(Some(result));
	}

	fn get_type_with_key<T>(&self, pref: PRef) -> Result<Option<(SHA256D, T)>, storage::Error>
	where
		T: Deserializable,
	{
		let (k, v) = self.get(pref)?;
		let hash = sha256d(k)?;
		let result = deserialize::<&[u8], T>(&v).map_err(from_serial)?;
		Ok(Some((hash, result)))
	}

	fn get_key(&self, pref: PRef) -> Result<SHA256D, storage::Error> {
		sha256d(self.hammersbald.read().get(pref).map_err(from_ham)?.0)
	}

	fn put_keyed(&self, key: &[u8], data: &[u8]) -> Result<PRef, storage::Error> {
		self.hammersbald.write().put_keyed(key, data).map_err(from_ham)
	}

	fn get_keyed(&self, key: &[u8]) -> Result<Option<(PRef, Vec<u8>)>, storage::Error> {
		self.hammersbald.read().get_keyed(key).map_err(from_ham)
	}

	fn put(&self, data: &[u8]) -> Result<PRef, storage::Error> {
		self.hammersbald.write().put(data).map_err(from_ham)
	}

	fn get(&self, pref: PRef) -> Result<(Vec<u8>, Vec<u8>), storage::Error> {
		self.hammersbald.read().get(pref).map_err(from_ham)
	}

	fn set(&self, pref: PRef, data: &[u8]) -> Result<PRef, storage::Error> {
		self.hammersbald.write().set(pref, data).map_err(from_ham)
	}
}

impl DbInterface for HamDb {
	fn insert_block(&self, block: IndexedBlock) -> Result<(), storage::Error> {
		debug!("Inserting block header: {} {:?}", block.header.hash, block.header.raw);
		let header_pref = self.put(&serialize(&block.header.raw))?;

		let mut tx_refs: Vec<PRef> = vec![];

		for tx in &block.transactions {
			let tx_meta = TransactionMeta::new(0, tx.raw.outputs.len());
			debug!("Inserting transaction meta: {} {:?}", tx.hash, tx_meta);
			let meta_pref = self.put(&serialize(&tx_meta))?;
			debug!("Inserting transaction: {} {:?}", tx.hash, tx.raw);
			let tx_pref = self.put(&serialize(&tx.raw))?;
			let db_tx = DbTransaction::new(tx_pref, meta_pref);
			let db_tx_pref = self.put_keyed(&serialize(&tx.hash), &serialize(&db_tx))?;

			for input in &tx.raw.inputs {
				let input = DbInputKey::from(input);
				self.put_keyed(input.as_ref(), &serialize(&db_tx_pref))?;
			}
			for output in &tx.raw.outputs {
				let (key, value) = DbOutput::from(output, db_tx_pref);
				self.put_keyed(key.as_ref(), &serialize(&value))?;
			}
			tx_refs.push(db_tx_pref);
		}
		let mut meta = BlockMeta::default();
		meta.n_tx = block.transactions.len() as u32;

		let meta_pref = self.put(&serialize(&meta))?;

		let db_block = DbBlock {
			header: header_pref,
			txs: tx_refs,
			meta: meta_pref,
		};

		debug!("Inserting db_block: {} {:?}", block.header.hash, db_block);
		self.put_keyed(&serialize(&block.header.hash), &serialize(&db_block))?;
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
		if let Some((_, db_block)) = self.get_keyed_type::<DbBlock>(block_hash)? {
			return Ok(self.get_type::<BlockHeader>(db_block.header)?.map(|header| header.into()));
		}
		Ok(None)
	}

	fn fetch_block_meta(&self, block_hash: &SHA256D) -> Result<Option<BlockMeta>, storage::Error> {
		if let Some((_, db_block)) = self.get_keyed_type::<DbBlock>(block_hash)? {
			let result = self.get_type::<BlockMeta>(db_block.meta);
			debug!("fetched meta: {:?} {:?}", block_hash, result);
			return result;
		}
		Ok(None)
	}

	fn fetch_transactions(&self, block_hash: &SHA256D) -> Result<Option<Vec<IndexedTransaction>>, storage::Error> {
		if let Some((_, db_block)) = self.get_keyed_type::<DbBlock>(block_hash)? {
			let mut txs = Vec::new();
			for tx_ref in &db_block.txs {
				if let Some(db_tx) = self.get_type::<DbTransaction>(*tx_ref)? {
					if let Some(tx) = self.get_type::<Transaction>(db_tx.tx_pref)? {
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
		if let Some((_, db_block)) = self.get_keyed_type::<DbBlock>(block_hash)? {
			let mut tx_hashes = Vec::new();
			for tx_ref in &db_block.txs {
				if let Some((k, _)) = self.get_type_with_key::<DbTransaction>(*tx_ref)? {
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
		if let Some((_, db_tx)) = self.get_keyed_type::<DbTransaction>(tx_hash)? {
			if let Some(tx) = self.get_type::<Transaction>(db_tx.tx_pref)? {
				return Ok(Some(tx.into()));
			}
		}
		Ok(None)
	}

	fn fetch_transaction_meta(&self, tx_hash: &SHA256D) -> Result<Option<TransactionMeta>, storage::Error> {
		if let Some((_, db_tx)) = self.get_keyed_type::<DbTransaction>(tx_hash)? {
			return self.get_type::<TransactionMeta>(db_tx.meta_pref);
		}
		Ok(None)
	}

	fn update_block_meta(&self, block_hash: &SHA256D, block_meta: &BlockMeta) -> Result<(), storage::Error> {
		debug!("Update block meta: {} {:?}", block_hash, block_meta);
		if let Some((_, db_block)) = self.get_keyed_type::<DbBlock>(block_hash)? {
			self.set(db_block.meta, &serialize(block_meta))?;
			return Ok(());
		}
		Err(storage::Error::InconsistentData)
	}

	fn update_transaction_meta(&self, tx_hash: &SHA256D, tx_meta: TransactionMeta) -> Result<(), storage::Error> {
		debug!("Update tx meta: {} {:?}", tx_hash, tx_meta);
		if let Some((_, db_tx)) = self.get_keyed_type::<DbTransaction>(tx_hash)? {
			self.set(db_tx.meta_pref, &serialize(&tx_meta))?;
			return Ok(());
		}
		Err(storage::Error::InconsistentData)
	}

	fn set_block_by_number(&self, block_hash: &SHA256D, number: u32) -> Result<(), storage::Error> {
		debug!("Set block number: {} {:?}", block_hash, number);
		if let Some((block_pref, _)) = self.get_keyed_type::<DbBlock>(block_hash)? {
			match self.get_keyed(&serialize(&number))? {
				Some((pref, _)) => self.set(pref, &serialize(&block_pref))?,
				None => self.put_keyed(&serialize(&number), &serialize(&block_pref))?,
			};
			return Ok(());
		}
		Err(storage::Error::InconsistentData)
	}

	fn block_hash(&self, number: u32) -> Result<Option<SHA256D>, storage::Error> {
		if let Some((_, v)) = self.get_keyed(&serialize(&number))? {
			let pref = deserialize::<&[u8], PRef>(&v).map_err(from_serial)?;
			return Ok(Some(self.get_key(pref)?));
		}
		Ok(None)
	}

	fn best_block(&self) -> Result<BlockHeight, storage::Error> {
		match self.get_type::<PRef>(Self::best_pref()) {
			Ok(Some(best_pref)) => {
				let hash = self.get_key(best_pref)?;
				if let Some(meta) = self.fetch_block_meta(&hash)? {
					return Ok(BlockHeight { hash, number: meta.number });
				}
				return Err(storage::Error::DatabaseError("Failed to fetch best block".to_string()));
			}
			_ => {
				let pref = self.put(&serialize(&PRef::invalid()))?;
				if pref != Self::best_pref() {
					return Err(storage::Error::DatabaseError("Database initialisation error".to_string()));
				}
				self.flush()?;
				Ok(BlockHeight::default())
			}
		}
	}

	fn set_best(&self, block_hash: &SHA256D) -> Result<(), storage::Error> {
		if let Some((db_block_pref, _)) = self.get_keyed_type::<DbBlock>(block_hash)? {
			self.set(Self::best_pref(), &serialize(&db_block_pref))?;
			return Ok(());
		}
		Err(storage::Error::CannotCanonize)
	}

	fn flush(&self) -> Result<(), storage::Error> {
		self.hammersbald.write().batch().map_err(from_ham)
	}
}

fn from_ham(e: hammersbald::Error) -> storage::Error {
	storage::Error::DatabaseError(e.to_string())
}

fn from_serial(e: serialization::Error) -> storage::Error {
	storage::Error::DatabaseError(e.to_string())
}

fn sha256d(bytes: Vec<u8>) -> Result<SHA256D, storage::Error> {
	SHA256D::from_slice(&bytes).map_err(|_| storage::Error::DatabaseError("error deserialising hash".to_string()))
}

impl Clone for HamDb {
	fn clone(&self) -> Self {
		HamDb {
			hammersbald: self.hammersbald.clone(),
		}
	}
}
