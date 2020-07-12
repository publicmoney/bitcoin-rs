use crate::db_block::{DbBlockHeader, DbInputKey, DbOutput, DbTransaction};
use bitcrypto::{Hash, SHA256D};
use chain::{BlockHeader, IndexedBlock, IndexedBlockHeader, IndexedTransaction};
use hammersbald::{HammersbaldAPI, PRef};
use parking_lot::RwLock;
use serialization::{deserialize, serialize, Deserializable, Serializable};
use std::collections::HashMap;
use std::error::Error;
use storage::{BlockHeight, BlockMeta, BlockOrigin, BlockRef, SideChainOrigin, TransactionMeta};
// use crate::db_block::{DbIndexedBlock, DbIndexedBlockHeader, DbBlockHeader};

const BEST_BLOCK: &str = "BEST_BLOCK";
const MAX_FORK_ROUTE_PRESET: usize = 2048;

pub struct BlockChainDatabase {
	hammersbald: Box<dyn HammersbaldAPI>,
	best_block: RwLock<BlockHeight>,
}

// TODO use custom error. no unwrap

pub fn from_ham(e: hammersbald::Error) -> storage::Error {
	storage::Error::DatabaseError(e.to_string())
}

pub fn from_serial(e: serialization::Error) -> storage::Error {
	storage::Error::DatabaseError(e.to_string())
}

impl BlockChainDatabase {
	pub fn open(mut hammersbald: Box<dyn HammersbaldAPI>) -> Result<BlockChainDatabase, storage::Error> {
		let best_block = match hammersbald.get_keyed(BEST_BLOCK.as_ref()).map_err(from_ham)? {
			Some((_, data)) => {
				let pref = deserialize(data.as_slice()).map_err(from_serial)?;
				let (key, data) = hammersbald.get(pref).map_err(from_ham)?;
				let db_header: DbBlockHeader = deserialize(data.as_slice()).unwrap();
				BlockHeight {
					hash: SHA256D::from_slice(&key).unwrap(),
					number: db_header.number,
				}
			}
			None => BlockHeight::default(),
		};
		Ok(BlockChainDatabase {
			hammersbald,
			best_block: RwLock::new(best_block),
		})
	}

	pub fn insert(&mut self, block: &IndexedBlock) -> Result<(), storage::Error> {
		let mut db_block = DbBlockHeader::from(&block, self)?;
		let hash = block.header.hash;

		let mut tx_refs: Vec<PRef> = vec![];
		for t in &block.transactions {
			let db_tx = DbTransaction::from(t);
			let tx_ref = self.hammersbald.put_keyed(&t.hash[..], &serialize(&db_tx)).map_err(from_ham)?;

			for i in db_tx.inputs {
				let input = DbInputKey::from(&i);
				self.hammersbald
					.put_keyed(&serialize(&input), &serialize(&tx_ref))
					.map_err(from_ham)?;
			}
			for o in db_tx.outputs {
				let output = DbOutput::from(o, tx_ref);
				self.hammersbald
					.put_keyed(&serialize(&output.key), &serialize(&output.value))
					.map_err(from_ham)?;
			}
			tx_refs.push(tx_ref);
		}
		let txs_pref = self.hammersbald.put(&serialize(&tx_refs)).map_err(from_ham)?;
		db_block.txs = txs_pref;

		let _pref = self.hammersbald.put_keyed(&hash[..], &serialize(&db_block)).map_err(from_ham)?;
		self.hammersbald.batch().map_err(from_ham)?;
		// println!("Stored block with hash: {:?} {:?}", block.header.hash, pref);
		Ok(())
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
						.block_header(&next_hash)?
						.expect("not to find orphaned side chain in database; qed")
						.0
						.raw
						.previous_header_hash;
				}
			}
		}

		Err(storage::Error::AncientFork)
	}

	pub fn block_header(&self, hash: &SHA256D) -> Result<Option<(IndexedBlockHeader, BlockMeta)>, storage::Error> {
		if let Some((pref, db_header)) = self.get_keyed::<DbBlockHeader>(&hash) {
			debug!("Fetching block header with hash {:?} {:?}", hash, pref);
			let header = db_header.to_block(self);

			let meta = BlockMeta {
				number: db_header.number,
				n_tx: db_header.n_txs,
				n_chain_tx: db_header.n_chain_txs,
			};
			return Ok(Some((header.into(), meta)));
		}
		Ok(None)
	}

	pub fn canonize(&mut self, block_hash: &SHA256D) -> Result<(), storage::Error> {
		// println!("Try to canonize {:?}", block_hash);
		let mut best_block = self.best_block.write();

		let (new_pref, mut new_best_db_block) = match self.get_keyed::<DbBlockHeader>(block_hash) {
			Some(b) => b,
			_ => {
				error!(target: "db", "Block is not found during canonization: {}", block_hash);
				return Err(storage::Error::CannotCanonize);
			}
		};

		if best_block.hash != SHA256D::default() {
			let current_db_block: DbBlockHeader = self.get_keyed(&best_block.hash).unwrap().1;
			new_best_db_block.number = current_db_block.number + 1;
			new_best_db_block.n_chain_txs = current_db_block.n_chain_txs + new_best_db_block.n_txs;
		}

		self.hammersbald
			.put_keyed(&block_hash[..], &serialize(&new_best_db_block))
			.map_err(from_ham)?;
		self.hammersbald
			.put_keyed(serialize(&new_best_db_block.number).as_ref(), &serialize(&new_pref));
		self.hammersbald.put_keyed(BEST_BLOCK.as_ref(), &serialize(&new_pref));

		let mut modified_txs: HashMap<SHA256D, (PRef, DbTransaction)> = HashMap::new();
		let transactions = self.db_block_transactions(new_best_db_block.txs);

		if let Some((hash, tx_ref, tx)) = transactions.first() {
			let mut modified_tx = tx.clone();
			modified_tx.height = new_pref;
			modified_tx.set_coinbase();
			modified_txs.insert(*hash, (*tx_ref, modified_tx));
		}

		for (hash, tx_ref, tx) in transactions.iter().skip(1) {
			modified_txs.insert(*hash, (*tx_ref, tx.clone()));

			for input in &tx.inputs {
				use std::collections::hash_map::Entry;

				match modified_txs.entry(input.previous_output.hash.clone()) {
					Entry::Occupied(mut entry) => {
						let (_tx_ref, tx) = entry.get_mut();
						tx.denote_used(input.previous_output.index as usize);
					}
					Entry::Vacant(entry) => {
						let (tx_ref, mut tx) = self.get_keyed::<DbTransaction>(&input.previous_output.hash).ok_or_else(|| {
							error!(
								target: "db",
								"Cannot find tx during canonization of tx {}: {}/{}",
								hash,
								input.previous_output.hash,
								input.previous_output.index,
							);
							storage::Error::CannotCanonize
						})?;
						tx.denote_used(input.previous_output.index as usize);
						entry.insert((tx_ref, tx));
					}
				}
			}
		}

		for (tx_ref, tx) in modified_txs.values() {
			self.hammersbald.set(*tx_ref, &serialize(tx));
		}
		self.hammersbald.batch().map_err(from_ham)?;

		// println!("Canonized: {:?}", block_hash);
		*best_block = BlockHeight {
			hash: *block_hash,
			number: new_best_db_block.number,
		};

		Ok(())
	}

	fn rollback_best(&self) -> Result<SHA256D, storage::Error> {
		unimplemented!()
	}

	pub fn best_block(&self) -> BlockHeight {
		self.best_block.read().clone()
	}

	pub fn get_ref(&self, hash: &SHA256D) -> Result<Option<PRef>, storage::Error> {
		Ok(self.hammersbald.get_keyed(&hash[..]).map_err(from_ham)?.map(|kv| kv.0))
	}

	pub fn get_key(&self, pref: PRef) -> Result<SHA256D, Box<dyn Error>> {
		Ok(SHA256D::from_slice(self.hammersbald.get(pref)?.0.as_ref())?)
	}

	fn get_keyed<T>(&self, hash: &SHA256D) -> Option<(PRef, T)>
	where
		T: Deserializable,
	{
		self.hammersbald
			.get_keyed(&hash[..])
			.expect("unexpected database error")
			.and_then(|(k, v)| Some((k, deserialize::<&[u8], T>(v.as_ref()).expect("unexpected database error"))))
	}

	fn get<T>(&self, pref: PRef) -> Option<T>
	where
		T: Deserializable,
	{
		self.hammersbald
			.get(pref)
			.and_then(|(_, v)| Ok(deserialize::<&[u8], T>(v.as_ref()).unwrap()))
			.ok()
	}

	fn get_key_value<T>(&self, pref: PRef) -> Option<(SHA256D, T)>
	where
		T: Deserializable,
	{
		self.hammersbald
			.get(pref)
			.and_then(|(k, v)| {
				Ok((
					SHA256D::from_slice(k.as_ref()).unwrap(),
					deserialize::<&[u8], T>(v.as_ref()).unwrap(),
				))
			})
			.ok()
	}

	fn block(&self, block_ref: BlockRef) -> Option<IndexedBlock> {
		self.resolve_hash(block_ref).and_then(|block_hash| {
			self.block_header(&block_hash).unwrap().map(|(header, meta)| {
				let transactions = self.block_transactions(block_hash.into());
				IndexedBlock::new(header, transactions)
			})
		})
	}

	pub fn transaction(&self, hash: &SHA256D) -> Option<IndexedTransaction> {
		self.get_keyed::<DbTransaction>(hash).map(|(pref, db_tx)| db_tx.to_indexed())
	}

	fn block_transaction_hashes(&self, block_ref: BlockRef) -> Vec<SHA256D> {
		self.resolve_hash(block_ref)
			.and_then(|hash| self.get_keyed::<DbBlockHeader>(&hash))
			.and_then(|(_, header)| self.get::<Vec<PRef>>(header.txs))
			.and_then(|tx_refs| Some(tx_refs.iter().map(|tx_ref| self.get_key(*tx_ref).unwrap()).collect()))
			.unwrap_or_default()
	}

	pub fn db_block_transactions(&self, txs_ref: PRef) -> Vec<(SHA256D, PRef, DbTransaction)> {
		self.get::<Vec<PRef>>(txs_ref)
			.and_then(|tx_refs| {
				Some(
					tx_refs
						.iter()
						.map(|tx_ref| {
							let (k, v) = self.get_key_value::<DbTransaction>(*tx_ref).unwrap();
							(k, *tx_ref, v)
						})
						.collect(),
				)
			})
			.unwrap_or_default()
	}

	pub fn block_transactions(&self, block_ref: BlockRef) -> Vec<IndexedTransaction> {
		self.block_transaction_hashes(block_ref)
			.into_iter()
			.filter_map(|hash| self.transaction(&hash))
			.collect()
	}

	pub fn resolve_hash(&self, block_ref: BlockRef) -> Option<SHA256D> {
		match block_ref {
			BlockRef::Number(n) => self.block_hash(n),
			BlockRef::Hash(h) => Some(h),
		}
	}

	pub fn block_hash(&self, number: u32) -> Option<SHA256D> {
		self.hammersbald
			.get_keyed(&serialize(&number))
			.unwrap()
			.map(|(_, data)| self.get_key(deserialize(data.as_slice()).unwrap()).unwrap())
	}

	pub fn block_number(&self, hash: &SHA256D) -> Option<u32> {
		self.get_keyed::<DbBlockHeader>(&hash).map(|(_, h)| h.number)
	}

	fn contains_block(&self, block_ref: BlockRef) -> bool {
		self.resolve_hash(block_ref)
			.and_then(|hash| self.get_keyed::<DbBlockHeader>(&hash))
			.is_some()
	}

	// /// get blockchain difficulty
	// pub fn difficulty(&self) -> f64 {
	// 	let best = &*self.best_block.unwrap_or_default().read().unwrap();
	// 	match best {
	// 		Some((pref, header)) => header.bits.to_f64(),
	// 		_ => 0f64
	// 	}
	// }
}

#[cfg(test)]
mod tests {
	use super::SHA256D;
	use crate::adapator::BlockChainDatabase;
	use chain::IndexedBlock;
	use hammersbald::{persistent, transient};
	use storage::BlockRef;
	use test_data::{block_h0, block_h1, block_h2};

	#[test]
	pub fn bitcoin_test() {
		let hammersbald = transient(1).unwrap();
		// let db = persistent("test", 1, 1).unwrap();
		let mut db = BlockChainDatabase::open(hammersbald).unwrap();

		let b0: IndexedBlock = block_h0().into();
		db.insert(&b0).unwrap();
		db.canonize(b0.hash()).unwrap();

		assert_best(&db, 0, b0.header.hash);

		let b1: IndexedBlock = block_h1().into();
		db.insert(&b1).unwrap();
		db.canonize(b1.hash()).unwrap();

		assert_best(&db, 1, b1.header.hash);

		let b2: IndexedBlock = block_h2().into();
		db.insert(&b2).unwrap();
		db.canonize(b2.hash()).unwrap();

		assert_best(&db, 2, b2.header.hash);

		assert_eq!(db.block_hash(0).unwrap(), b0.header.hash);
		assert_eq!(db.block_hash(1).unwrap(), b1.header.hash);
		assert_eq!(db.block_hash(2).unwrap(), b2.header.hash);

		let s2 = db.block(BlockRef::Hash(*b2.hash())).unwrap();

		assert_eq!(s2.hash(), &b2.header.hash);
		assert_eq!(s2.transactions.len(), 1);
		assert_eq!(s2.header.raw.previous_header_hash, b1.header.hash);
		assert_eq!(s2.header.raw.merkle_root_hash, b2.header.raw.merkle_root_hash);
		assert_eq!(s2.header.raw.version, b2.header.raw.version);
		assert_eq!(s2.header.raw.time, b2.header.raw.time);
		assert_eq!(s2.header.raw.bits, b2.header.raw.bits);
		assert_eq!(s2.header.raw.nonce, b2.header.raw.nonce);
	}

	fn assert_best(db: &BlockChainDatabase, number: u32, hash: SHA256D) {
		let best = db.best_block();
		assert_eq!(best.number, number);
		assert_eq!(best.hash, hash);
	}

	#[test]
	pub fn update_test() {
		// let mut db = persistent("deletetest", 1, 1).unwrap();
		let mut db = transient(1).unwrap();
		let pref = db.put(&[1]).unwrap();
		let updated_pref = db.set(pref, &[7]).unwrap();
		db.batch();
		assert_eq!(pref, updated_pref);
		let vec = db.get(pref).unwrap().1;
		assert_eq!(vec, &[7])
	}
}
