use crate::api::{Hammersbald, HammersbaldAPI};
use crate::async_file::AsyncFile;
use crate::cached_file::CachedFile;
use crate::data_file::DataFile;
use crate::error::Error;
use crate::log_file::LogFile;
use crate::page::PAGE_SIZE;
use crate::rolled_file::RolledFile;
use crate::table_file::TableFile;

const TABLE_FILE_SIZE: u64 = 262_144 * PAGE_SIZE as u64;
const DATA_FILE_SIZE: u64 = 262_144 * PAGE_SIZE as u64;
const LOG_FILE_SIZE: u64 = 262_144 * PAGE_SIZE as u64;

/// Implements persistent storage
pub struct Persistent {}

impl Persistent {
	/// create a new db
	pub fn new_db(name: &str, cache_size_mb: usize) -> Result<Box<dyn HammersbaldAPI>, Error> {
		let data = DataFile::new(Box::new(CachedFile::new(
			Box::new(AsyncFile::new(Box::new(RolledFile::new(name, "bc", DATA_FILE_SIZE)?))?),
			cache_size_mb,
		)?))?;

		let link = DataFile::new(Box::new(CachedFile::new(
			Box::new(AsyncFile::new(Box::new(RolledFile::new(name, "bl", DATA_FILE_SIZE)?))?),
			cache_size_mb,
		)?))?;

		let log = LogFile::new(Box::new(AsyncFile::new(Box::new(RolledFile::new(name, "lg", LOG_FILE_SIZE)?))?));

		let table = TableFile::new(Box::new(CachedFile::new(
			Box::new(RolledFile::new(name, "tb", TABLE_FILE_SIZE)?),
			cache_size_mb,
		)?))?;

		Ok(Box::new(Hammersbald::new(log, table, data, link)?))
	}
}

#[cfg(test)]
mod test {
	use crate::persistent::Persistent;

	#[test]
	#[allow(unused_must_use)]
	fn test_reopen_persistent() {
		std::fs::remove_file("test.0.bc");
		std::fs::remove_file("test.0.lg");
		std::fs::remove_file("test.0.tb");
		std::fs::remove_file("test.0.bl");

		let key = "abc".as_bytes();
		let expected_pref = 0;
		let value = [1, 2, 3];
		{
			let mut db = Persistent::new_db("test", 1).unwrap();
			let pref = db.put_keyed(key, &value).unwrap();
			assert_eq!(pref, expected_pref);
			db.batch().unwrap();
		}

		let db = Persistent::new_db("test", 1).unwrap();
		let (pref, result) = db.get_keyed(key).unwrap().unwrap();
		assert_eq!(pref, expected_pref);
		assert_eq!(value, result.as_slice());
	}
}
