//! Key-Value store abstraction with `RocksDB` backend.

use bytes::Bytes;
use kv::{Key, KeyState, KeyValueDatabase, Location, RawKey, RawKeyValue, RawOperation, RawTransaction, Transaction, Value};
use rocksdb::{BlockBasedOptions, DBCompactionStyle, DBIterator, IteratorMode, Options, ReadOptions, WriteBatch, WriteOptions, DB};
use std::collections::HashMap;
use std::path::Path;

const DB_BACKGROUND_FLUSHES: i32 = 2;
const DB_BACKGROUND_COMPACTIONS: i32 = 2;

/// Compaction profile for the database settings
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CompactionProfile {
	/// L0-L1 target file size
	pub initial_file_size: u64,
	/// rate limiter for background flushes and compactions, bytes/sec, if any
	pub write_rate_limit: Option<u64>,
}

impl Default for CompactionProfile {
	/// Default profile suitable for most storage
	fn default() -> CompactionProfile {
		CompactionProfile::ssd()
	}
}

impl CompactionProfile {
	/// Default profile suitable for SSD storage
	pub fn ssd() -> CompactionProfile {
		CompactionProfile {
			initial_file_size: 32 * 1024 * 1024,
			write_rate_limit: None,
		}
	}

	/// Slow HDD compaction profile
	pub fn hdd() -> CompactionProfile {
		CompactionProfile {
			initial_file_size: 192 * 1024 * 1024,
			write_rate_limit: Some(8 * 1024 * 1024),
		}
	}
}

/// Database configuration
#[derive(Clone)]
pub struct DatabaseConfig {
	/// Max number of open files.
	pub max_open_files: i32,
	/// Cache sizes (in MiB) for specific columns.
	pub cache_sizes: HashMap<Option<u32>, usize>,
	/// Specific number of bloom filter bits, if any
	pub bloom_filters: HashMap<Option<u32>, u8>,
	/// Compaction profile
	pub compaction: CompactionProfile,
	/// Set number of columns
	pub columns: Option<u32>,
	/// Should we keep WAL enabled?
	pub wal: bool,
}

impl DatabaseConfig {
	/// Create new `DatabaseConfig` with default parameters and specified set of columns.
	/// Note that cache sizes must be explicitly set.
	pub fn with_columns(columns: Option<u32>) -> Self {
		let mut config = Self::default();
		config.columns = columns;
		config
	}

	/// Set the column cache size in MiB.
	pub fn set_cache(&mut self, col: Option<u32>, size: usize) {
		self.cache_sizes.insert(col, size);
	}
}

impl Default for DatabaseConfig {
	fn default() -> DatabaseConfig {
		DatabaseConfig {
			cache_sizes: HashMap::new(),
			bloom_filters: HashMap::new(),
			max_open_files: 512,
			compaction: CompactionProfile::default(),
			columns: None,
			wal: true,
		}
	}
}

/// Database iterator for flushed data only
pub struct DatabaseIterator<'a> {
	iter: DBIterator<'a>,
}

impl Iterator for DatabaseIterator<'_> {
	type Item = (Box<[u8]>, Box<[u8]>);

	fn next(&mut self) -> Option<Self::Item> {
		self.iter.next()
	}
}

/// Key-Value database.
pub struct Database {
	db: DB,
	write_opts: WriteOptions,
	read_opts: ReadOptions,
}

impl KeyValueDatabase for Database {
	fn write(&self, tx: Transaction) -> Result<(), String> {
		Database::write(self, (&tx).into())
	}

	fn get(&self, key: &Key) -> Result<KeyState<Value>, String> {
		match Database::get(self, &key.into())? {
			Some(value) => Ok(KeyState::Insert(Value::for_key(key, &value)?)),
			None => Ok(KeyState::Unknown),
		}
	}
}

impl Database {
	/// Open database with default settings.
	pub fn open_default<P>(path: P) -> Result<Database, String>
	where
		P: AsRef<Path>,
	{
		Database::open(DatabaseConfig::default(), path)
	}

	/// Open database file. Creates if it does not exist.
	pub fn open<P>(config: DatabaseConfig, path: P) -> Result<Database, String>
	where
		P: AsRef<Path>,
	{
		// default cache size for columns not specified.
		const DEFAULT_CACHE: usize = 2;

		let mut opts = Options::default();
		if let Some(rate_limit) = config.compaction.write_rate_limit {
			opts.set_bytes_per_sync(rate_limit);
		}
		//		opts.set_max_total_wal_size(64 * 1024 * 1024); TODO enable when 0.14 is released
		opts.set_max_open_files(config.max_open_files);
		opts.create_if_missing(true);
		opts.set_use_fsync(false);

		opts.set_max_background_flushes(DB_BACKGROUND_FLUSHES);
		opts.set_max_background_compactions(DB_BACKGROUND_COMPACTIONS);

		// compaction settings
		opts.set_compaction_style(DBCompactionStyle::Universal);
		opts.set_target_file_size_base(config.compaction.initial_file_size);

		let mut cf_options = Vec::with_capacity(config.columns.unwrap_or(0) as usize);
		let cfnames: Vec<_> = (0..config.columns.unwrap_or(0)).map(|c| format!("col{}", c)).collect();
		let cfnames: Vec<&str> = cfnames.iter().map(|n| n as &str).collect();

		for col in 0..config.columns.unwrap_or(0) {
			let mut opts = Options::default();
			opts.set_compaction_style(DBCompactionStyle::Universal);
			opts.set_target_file_size_base(config.compaction.initial_file_size);

			let col_opt = config.columns.map(|_| col);

			{
				let mut block_opts = BlockBasedOptions::default();
				let cache_size = config.cache_sizes.get(&col_opt).cloned().unwrap_or(DEFAULT_CACHE);
				if let Some(bloom_bits) = config.bloom_filters.get(&col_opt) {
					block_opts.set_bloom_filter(bloom_bits.clone() as i32, false);
				}
				// all goes to read cache.
				block_opts.set_lru_cache(cache_size * 1024 * 1024);
				opts.set_block_based_table_factory(&block_opts);
			}

			cf_options.push(opts);
		}

		let mut write_opts = WriteOptions::new();
		if !config.wal {
			write_opts.disable_wal(true);
		}
		let mut read_opts = ReadOptions::default();
		read_opts.set_verify_checksums(false);

		let db = if !cfnames.is_empty() {
			match DB::open_cf(&opts, &path, &cfnames) {
				Ok(db) => Ok(db),
				Err(_) => {
					// retry and create CFs
					let empty_cfs: [&str; 0] = [];
					match DB::open_cf(&opts, &path, &empty_cfs) {
						Ok(mut db) => {
							cfnames
								.iter()
								.enumerate()
								.for_each(|(i, n)| db.create_cf(n, &cf_options[i]).unwrap());
							Ok(db)
						}
						err @ Err(_) => err,
					}
				}
			}
		} else {
			DB::open(&opts, &path)
		};

		let db = match db {
			Ok(db) => db,
			Err(e) if e.clone().into_string().starts_with("Corruption:") => {
				info!("{}", e.into_string());
				info!("Attempting DB repair for {}", path.as_ref().display());
				DB::repair(&opts, &path)?;

				match cfnames.is_empty() {
					true => DB::open(&opts, &path)?,
					false => DB::open_cf(&opts, &path, &cfnames)?,
				}
			}
			Err(e) => {
				return Err(e.into_string());
			}
		};
		Ok(Database { db, write_opts, read_opts })
	}

	/// Commit transaction to database.
	pub fn write(&self, tx: RawTransaction) -> Result<(), String> {
		let mut batch = WriteBatch::default();

		for op in tx.operations.into_iter() {
			match op {
				RawOperation::Insert(RawKeyValue { location, key, value }) => match location {
					Location::DB => batch.put(&key, &value)?,
					Location::Column(col) => {
						let cf = self.db.cf_handle(&format!("col{}", col)).expect("column not found");
						batch.put_cf(cf, &key, &value)?
					}
				},
				RawOperation::Delete(RawKey { location, key }) => match location {
					Location::DB => batch.delete(&key)?,
					Location::Column(col) => {
						let cf = self.db.cf_handle(&format!("col{}", col)).expect("column not found");
						batch.delete_cf(&cf, &key)?
					}
				},
			}
		}
		self.db.write_opt(batch, &self.write_opts).map_err(|e| e.into_string())
	}

	/// Get value by key.
	pub fn get(&self, key: &RawKey) -> Result<Option<Bytes>, String> {
		match key.location {
			Location::DB => {
				let value = self.db.get_opt(&key.key, &self.read_opts)?;
				Ok(value.map(|v| (&*v).into()))
			}
			Location::Column(col) => {
				let cf = self.db.cf_handle(&format!("col{}", col)).expect("column not found");
				let value = self.db.get_cf_opt(cf, &key.key, &self.read_opts)?;
				Ok(value.map(|v| (&*v).into()))
			}
		}
	}

	pub fn iter(&self, location: Location) -> DatabaseIterator {
		match location {
			Location::DB => DatabaseIterator {
				iter: self.db.iterator_opt(IteratorMode::Start, &self.read_opts),
			},
			Location::Column(column) => {
				let cf = self.db.cf_handle(&format!("col{}", column)).expect("column not found");
				DatabaseIterator {
					iter: self
						.db
						.iterator_cf_opt(cf, &self.read_opts, IteratorMode::Start)
						.expect("iterator params are valid; qed"),
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	extern crate tempdir;

	use self::tempdir::TempDir;
	use super::*;
	use kv::{Location, RawTransaction};

	fn test_db(config: DatabaseConfig) {
		let tempdir = TempDir::new("").unwrap();
		let db = Database::open(config, tempdir.path()).unwrap();

		let key1 = b"key1";
		let key2 = b"key2";
		let key3 = b"key3";

		let mut batch = RawTransaction::default();
		batch.insert_raw(Location::DB, key1, b"cat");
		batch.insert_raw(Location::DB, key2, b"dog");
		db.write(batch).unwrap();

		assert_eq!(&*db.get(&RawKey::new(Location::DB, key1 as &[u8])).unwrap().unwrap(), b"cat");

		let contents: Vec<_> = db.iter(Location::DB).collect();
		assert_eq!(contents.len(), 2);
		assert_eq!(&*contents[0].0, &*key1);
		assert_eq!(&*contents[0].1, b"cat");
		assert_eq!(&*contents[1].0, &*key2);
		assert_eq!(&*contents[1].1, b"dog");

		let mut batch = RawTransaction::default();
		batch.delete_raw(Location::DB, key1);
		db.write(batch).unwrap();

		assert_eq!(db.get(&RawKey::new(Location::DB, key1 as &[u8])).unwrap(), None);

		let mut batch = RawTransaction::default();
		batch.insert_raw(Location::DB, key1, b"cat");
		db.write(batch).unwrap();

		let mut transaction = RawTransaction::default();
		transaction.insert_raw(Location::DB, key3, b"elephant");
		transaction.delete_raw(Location::DB, key1);
		db.write(transaction).unwrap();
		assert_eq!(&*db.get(&RawKey::new(Location::DB, key3 as &[u8])).unwrap().unwrap(), b"elephant");
	}

	#[test]
	fn kvdb() {
		let tempdir = TempDir::new("").unwrap();
		let _ = Database::open_default(tempdir.path()).unwrap();
		test_db(DatabaseConfig::default());
	}
}
