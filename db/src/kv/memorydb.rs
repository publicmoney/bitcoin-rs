use crate::bytes::Bytes;
use crate::kv::{Key, KeyState, KeyValue, KeyValueDatabase, Operation, Transaction, Value};
use bitcrypto::SHA256D;
use chain::{BlockHeader, Transaction as ChainTransaction};
use parking_lot::RwLock;
use ser::List;
use std::collections::HashMap;
use std::mem::replace;
use std::sync::Arc;
use storage::{BlockMeta, TransactionMeta};

#[derive(Default, Debug)]
struct InnerDatabase {
	meta: HashMap<&'static str, KeyState<Bytes>>,
	block_hash: HashMap<u32, KeyState<SHA256D>>,
	block_meta: HashMap<SHA256D, KeyState<BlockMeta>>,
	block_header: HashMap<SHA256D, KeyState<BlockHeader>>,
	block_transactions: HashMap<SHA256D, KeyState<List<SHA256D>>>,
	transaction: HashMap<SHA256D, KeyState<ChainTransaction>>,
	transaction_meta: HashMap<SHA256D, KeyState<TransactionMeta>>,
	configuration: HashMap<&'static str, KeyState<Bytes>>,
}

#[derive(Default, Debug)]
pub struct MemoryDatabase {
	db: RwLock<InnerDatabase>,
}

impl MemoryDatabase {
	pub fn drain_transaction(&self) -> Transaction {
		let mut db = self.db.write();
		let meta = replace(&mut db.meta, HashMap::default())
			.into_iter()
			.flat_map(|(key, state)| state.into_operation(key, KeyValue::Meta, Key::Meta));

		let block_hash = replace(&mut db.block_hash, HashMap::default())
			.into_iter()
			.flat_map(|(key, state)| state.into_operation(key, KeyValue::BlockHash, Key::BlockHash));

		let block_header = replace(&mut db.block_header, HashMap::default())
			.into_iter()
			.flat_map(|(key, state)| state.into_operation(key, KeyValue::BlockHeader, Key::BlockHeader));

		let block_transactions = replace(&mut db.block_transactions, HashMap::default())
			.into_iter()
			.flat_map(|(key, state)| state.into_operation(key, KeyValue::BlockTransactions, Key::BlockTransactions));

		let transaction = replace(&mut db.transaction, HashMap::default())
			.into_iter()
			.flat_map(|(key, state)| state.into_operation(key, KeyValue::Transaction, Key::Transaction));

		let transaction_meta = replace(&mut db.transaction_meta, HashMap::default())
			.into_iter()
			.flat_map(|(key, state)| state.into_operation(key, KeyValue::TransactionMeta, Key::TransactionMeta));

		let block_meta = replace(&mut db.block_meta, HashMap::default())
			.into_iter()
			.flat_map(|(key, state)| state.into_operation(key, KeyValue::BlockMeta, Key::BlockMeta));

		let configuration = replace(&mut db.configuration, HashMap::default())
			.into_iter()
			.flat_map(|(key, state)| state.into_operation(key, KeyValue::Configuration, Key::Configuration));

		Transaction {
			operations: meta
				.chain(block_hash)
				.chain(block_header)
				.chain(block_transactions)
				.chain(transaction)
				.chain(transaction_meta)
				.chain(block_meta)
				.chain(configuration)
				.collect(),
		}
	}
}

impl KeyValueDatabase for MemoryDatabase {
	fn write(&self, tx: Transaction) -> Result<(), String> {
		let mut db = self.db.write();
		for op in tx.operations.into_iter() {
			match op {
				Operation::Insert(insert) => match insert {
					KeyValue::Meta(key, value) => {
						db.meta.insert(key, KeyState::Insert(value));
					}
					KeyValue::BlockHash(key, value) => {
						db.block_hash.insert(key, KeyState::Insert(value));
					}
					KeyValue::BlockMeta(key, value) => {
						db.block_meta.insert(key, KeyState::Insert(value));
					}
					KeyValue::BlockHeader(key, value) => {
						db.block_header.insert(key, KeyState::Insert(value));
					}
					KeyValue::BlockTransactions(key, value) => {
						db.block_transactions.insert(key, KeyState::Insert(value));
					}
					KeyValue::Transaction(key, value) => {
						db.transaction.insert(key, KeyState::Insert(value));
					}
					KeyValue::TransactionMeta(key, value) => {
						db.transaction_meta.insert(key, KeyState::Insert(value));
					}
					KeyValue::Configuration(key, value) => {
						db.configuration.insert(key, KeyState::Insert(value));
					}
				},
				Operation::Delete(delete) => match delete {
					Key::Meta(key) => {
						db.meta.insert(key, KeyState::Delete);
					}
					Key::BlockHash(key) => {
						db.block_hash.insert(key, KeyState::Delete);
					}
					Key::BlockMeta(key) => {
						db.block_meta.insert(key, KeyState::Delete);
					}
					Key::BlockHeader(key) => {
						db.block_header.insert(key, KeyState::Delete);
					}
					Key::BlockTransactions(key) => {
						db.block_transactions.insert(key, KeyState::Delete);
					}
					Key::Transaction(key) => {
						db.transaction.insert(key, KeyState::Delete);
					}
					Key::TransactionMeta(key) => {
						db.transaction_meta.insert(key, KeyState::Delete);
					}
					Key::Configuration(key) => {
						db.configuration.insert(key, KeyState::Delete);
					}
				},
			}
		}
		Ok(())
	}

	fn get(&self, key: &Key) -> Result<KeyState<Value>, String> {
		let db = self.db.read();
		let result = match *key {
			Key::Meta(ref key) => db.meta.get(key).cloned().unwrap_or_default().map(Value::Meta),
			Key::BlockHash(ref key) => db.block_hash.get(key).cloned().unwrap_or_default().map(Value::BlockHash),
			Key::BlockMeta(ref key) => db.block_meta.get(key).cloned().unwrap_or_default().map(Value::BlockMeta),
			Key::BlockHeader(ref key) => db.block_header.get(key).cloned().unwrap_or_default().map(Value::BlockHeader),
			Key::BlockTransactions(ref key) => db
				.block_transactions
				.get(key)
				.cloned()
				.unwrap_or_default()
				.map(Value::BlockTransactions),
			Key::Transaction(ref key) => db.transaction.get(key).cloned().unwrap_or_default().map(Value::Transaction),
			Key::TransactionMeta(ref key) => db
				.transaction_meta
				.get(key)
				.cloned()
				.unwrap_or_default()
				.map(Value::TransactionMeta),
			Key::Configuration(ref key) => db.configuration.get(key).cloned().unwrap_or_default().map(Value::Configuration),
		};

		Ok(result)
	}
}

#[derive(Debug)]
pub struct SharedMemoryDatabase {
	db: Arc<MemoryDatabase>,
}

impl Default for SharedMemoryDatabase {
	fn default() -> Self {
		SharedMemoryDatabase { db: Arc::default() }
	}
}

impl Clone for SharedMemoryDatabase {
	fn clone(&self) -> Self {
		SharedMemoryDatabase { db: self.db.clone() }
	}
}

impl KeyValueDatabase for SharedMemoryDatabase {
	fn write(&self, tx: Transaction) -> Result<(), String> {
		self.db.write(tx)
	}

	fn get(&self, key: &Key) -> Result<KeyState<Value>, String> {
		self.db.get(key)
	}
}
