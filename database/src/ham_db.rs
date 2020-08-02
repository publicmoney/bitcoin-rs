use crate::db_interface::DbInterface;
use crate::ham_types::{DbBlock, DbInputKey, DbOutput, DbTransaction};
use bitcrypto::{Hash, SHA256D};
use chain::{BlockHeader, IndexedBlock, IndexedBlockHeader, IndexedTransaction, Transaction};
use hammersbald::{persistent, transient, HammersbaldAPI, PRef};
use parking_lot::RwLock;
use serialization::{deserialize, serialize, Deserializable};
use std::sync::Arc;
use storage::{BlockHeight, BlockMeta, TransactionMeta};

pub struct HamDb {
	hammersbald: Arc<RwLock<Box<dyn HammersbaldAPI>>>,
}

impl HamDb {
	pub fn transient() -> Result<HamDb, storage::Error> {
		Ok(Self::new(transient(1).map_err(from_ham)?))
	}

	pub fn persistent() -> Result<HamDb, storage::Error> {
		let path = "testdb";
		if std::fs::read(path).is_err() {
			std::fs::create_dir(path).unwrap();
		}
		Ok(Self::new(persistent("testdb/bitcoin-rs", 100, 1).map_err(from_ham)?))
	}

	fn new(hammersbald: Box<dyn HammersbaldAPI>) -> HamDb {
		HamDb {
			hammersbald: Arc::new(RwLock::new(hammersbald)),
		}
	}

	fn best_pref() -> PRef {
		PRef::from(0)
	}

	fn get_keyedd<T>(&self, hash: &SHA256D) -> Result<Option<(PRef, T)>, storage::Error>
	where
		T: Deserializable,
	{
		Ok(self
			.get_keyed(&serialize(hash))?
			.and_then(|(k, v)| deserialize::<&[u8], T>(&v).map_err(from_serial).map(|v| (k, v)).ok()))
	}

	fn gett<T>(&self, pref: PRef) -> Result<Option<T>, storage::Error>
	where
		T: Deserializable,
	{
		self.get(pref).map(|(_, v)| deserialize::<&[u8], T>(&v).map_err(from_serial).ok())
	}

	fn gett_with_key<T>(&self, pref: PRef) -> Result<Option<(SHA256D, T)>, storage::Error>
	where
		T: Deserializable,
	{
		self.get(pref).and_then(|(k, v)| {
			deserialize::<&[u8], T>(&v)
				.map_err(from_serial)
				.map(|v| SHA256D::from_slice(&k).and_then(|hash| Ok((hash, v))).ok())
		})
	}

	fn get_key(&self, pref: PRef) -> Result<SHA256D, storage::Error> {
		Ok(SHA256D::from_slice(self.hammersbald.read().get(pref).map_err(from_ham)?.0.as_ref()).unwrap())
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
		let header_pref = self.put(&serialize(&block.header.raw))?;
		let mut tx_refs: Vec<PRef> = vec![];
		for tx in &block.transactions {
			let tx_meta = TransactionMeta::new(0, tx.raw.outputs.len());
			let meta_pref = self.put(&serialize(&tx_meta))?;
			let tx_pref = self.put(&serialize(&tx.raw))?;
			let db_tx = DbTransaction {
				meta: meta_pref,
				transaction: tx_pref,
			};
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

		self.put_keyed(&serialize(&block.header.hash), &serialize(&db_block))?;
		Ok(())
	}

	fn fetch_block(&self, block_hash: &SHA256D) -> Result<Option<IndexedBlock>, storage::Error> {
		Ok(self.fetch_block_header(block_hash)?.and_then(|header| {
			self.fetch_transactions(block_hash)
				.unwrap_or_default()
				.and_then(|transactions| Some(IndexedBlock::new(header, transactions)))
		}))
	}

	fn fetch_block_header(&self, block_hash: &SHA256D) -> Result<Option<IndexedBlockHeader>, storage::Error> {
		Ok(self
			.get_keyedd::<DbBlock>(block_hash)?
			.and_then(|(_, db_block)| self.gett::<BlockHeader>(db_block.header).unwrap_or_default())
			.and_then(|header| Some(IndexedBlockHeader::from_raw(header))))
	}

	fn fetch_block_meta(&self, block_hash: &SHA256D) -> Result<Option<BlockMeta>, storage::Error> {
		Ok(self
			.get_keyedd::<DbBlock>(block_hash)?
			.and_then(|(_, db_block)| self.gett::<BlockMeta>(db_block.meta).unwrap_or_default()))
	}

	fn fetch_transactions(&self, block_hash: &SHA256D) -> Result<Option<Vec<IndexedTransaction>>, storage::Error> {
		Ok(self.get_keyedd::<DbBlock>(block_hash)?.and_then(|(_, db_block)| {
			db_block
				.txs
				.iter()
				.map(|tx_ref| {
					self.gett::<DbTransaction>(*tx_ref)
						.unwrap_or_default()
						.and_then(|db_tx| self.gett::<Transaction>(db_tx.transaction).unwrap_or_default())
						.and_then(|t| Some(t.into()))
				})
				.collect()
		}))
	}

	fn fetch_transaction_hashes(&self, block_hash: &SHA256D) -> Result<Option<Vec<SHA256D>>, storage::Error> {
		Ok(self.get_keyedd::<DbBlock>(block_hash)?.and_then(|(_, db_block)| {
			db_block
				.txs
				.iter()
				.map(|tx_ref| {
					self.gett_with_key::<DbTransaction>(*tx_ref)
						.unwrap_or_default()
						.and_then(|(k, _db_tx)| Some(k))
				})
				.collect()
		}))
	}

	fn fetch_transaction(&self, tx_hash: &SHA256D) -> Result<Option<IndexedTransaction>, storage::Error> {
		Ok(self
			.get_keyedd::<DbTransaction>(tx_hash)?
			.and_then(|(_, db_tx)| self.gett::<Transaction>(db_tx.transaction).unwrap_or_default())
			.and_then(|tx| Some(tx.into())))
	}

	fn fetch_transaction_meta(&self, tx_hash: &SHA256D) -> Result<Option<TransactionMeta>, storage::Error> {
		Ok(self
			.get_keyedd::<DbTransaction>(tx_hash)?
			.and_then(|(_, db_tx)| self.gett::<TransactionMeta>(db_tx.meta).unwrap_or_default()))
	}

	fn update_block_meta(&self, block_hash: &SHA256D, block_meta: &BlockMeta) -> Result<(), storage::Error> {
		self.get_keyedd::<DbBlock>(block_hash)?
			.map(|(_, db_block)| self.set(db_block.meta, &serialize(block_meta)));
		Ok(())
	}

	fn update_transaction_meta(&self, tx_hash: &SHA256D, tx_meta: TransactionMeta) -> Result<(), storage::Error> {
		self.get_keyedd::<DbTransaction>(tx_hash)?
			.map(|(_, db_tx)| self.set(db_tx.meta, &serialize(&tx_meta)));
		Ok(())
	}

	fn set_block_by_number(&self, block_hash: &SHA256D, number: u32) -> Result<(), storage::Error> {
		let number_pref = self.get_keyed(&serialize(&number))?;
		let (block_pref, _) = self.get_keyedd::<DbBlock>(block_hash)?.unwrap();

		match number_pref {
			Some((pref, _)) => self.set(pref, &serialize(&block_pref)),
			None => self.put_keyed(&serialize(&number), &serialize(&block_pref)),
		}
		.map(|_| ())
	}

	fn block_hash(&self, number: u32) -> Result<Option<SHA256D>, storage::Error> {
		Ok(self
			.get_keyed(&serialize(&number))?
			.and_then(|(_, v)| deserialize::<&[u8], PRef>(&v).map_err(from_serial).ok())
			.and_then(|pref| self.get_key(pref).ok()))
	}

	fn best_block(&self) -> Result<BlockHeight, storage::Error> {
		match self.gett::<PRef>(Self::best_pref()) {
			Ok(Some(best_pref)) => self
				.get_key(best_pref)
				.ok()
				.and_then(|hash| {
					self.fetch_block_meta(&hash)
						.unwrap_or_default()
						.and_then(|meta| Some(BlockHeight { hash, number: meta.number }))
				})
				.ok_or(storage::Error::DatabaseError("Failed to fetch best block".to_string())),
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
		self.get_keyedd::<DbBlock>(block_hash)?
			.map_or(Err(storage::Error::NotFound), |(pref, _)| {
				self.set(Self::best_pref(), &serialize(&pref)).map(|_| ())
			})
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

impl Clone for HamDb {
	fn clone(&self) -> Self {
		HamDb {
			hammersbald: self.hammersbald.clone(),
		}
	}
}
