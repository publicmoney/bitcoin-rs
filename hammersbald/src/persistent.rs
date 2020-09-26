use crate::async_file::AsyncFile;
use crate::cached_file::CachedFile;
use crate::data_file::DataFile;
use crate::error::Error;
use crate::hammersbald_api::{Hammersbald, HammersbaldAPI};
use crate::log_file::LogFile;
use crate::page::PAGE_SIZE;
use crate::rolled_file::RolledFile;
use crate::table_file::TableFile;

const TABLE_FILE_SIZE: u64 = 262_144 * PAGE_SIZE as u64;
const DATA_FILE_SIZE: u64 = 262_144 * PAGE_SIZE as u64;
const LOG_FILE_SIZE: u64 = 262_144 * PAGE_SIZE as u64;

/// Implements persistent storage
pub fn persistent(path: &str, name: &str, cache_size_mb: usize) -> Result<Box<dyn HammersbaldAPI>, Error> {
	std::fs::create_dir_all(path).unwrap();

	let data = DataFile::new(Box::new(CachedFile::new(
		Box::new(AsyncFile::new(Box::new(RolledFile::new(path, name, "bc", DATA_FILE_SIZE)?))?),
		cache_size_mb,
	)?))?;

	let link = DataFile::new(Box::new(CachedFile::new(
		Box::new(AsyncFile::new(Box::new(RolledFile::new(path, name, "bl", DATA_FILE_SIZE)?))?),
		cache_size_mb,
	)?))?;

	let log = LogFile::new(Box::new(AsyncFile::new(Box::new(RolledFile::new(
		path,
		name,
		"lg",
		LOG_FILE_SIZE,
	)?))?));

	let table = TableFile::new(Box::new(CachedFile::new(
		Box::new(RolledFile::new(path, name, "tb", TABLE_FILE_SIZE)?),
		cache_size_mb,
	)?))?;

	Ok(Box::new(Hammersbald::new(path, log, table, data, link)?))
}

#[cfg(test)]
mod test {
	use super::persistent;

	#[test]
	fn test_reopen_persistent() {
		let path = "testdb/persistent";
		std::fs::remove_dir_all(path).unwrap_or_default();

		let key = "abc".as_bytes();
		let expected_pref = 0;
		let value = [1, 2, 3];
		{
			let mut db = persistent(path, "test", 1).unwrap();
			let pref = db.put_keyed(key, &value).unwrap();
			assert_eq!(pref, expected_pref);
			db.batch().unwrap();
		}

		let db = persistent(path, "test", 1).unwrap();
		let (pref, result) = db.get_keyed(key).unwrap().unwrap();
		assert_eq!(pref, expected_pref);
		assert_eq!(value, result.as_slice());
	}
}
