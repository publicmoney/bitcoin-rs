use crate::async_file::AsyncFile;
use crate::cached_file::CachedFile;
use crate::data_file::DataFile;
use crate::error::Error;
use crate::hammersbald_api::{Hammersbald, HammersbaldAPI};
use crate::log_file::LogFile;
use crate::page::PAGE_SIZE;
use crate::rolled_file::RolledFile;
use crate::table_file::TableFile;

// Small files for tests, large files for real.
const fn file_size() -> u64 {
	(if option_env!("CARGO_PRIMARY_PACKAGE").is_none() {
		2 * PAGE_SIZE
	} else {
		262_1440 * PAGE_SIZE
	}) as u64
}

/// Implements persistent storage
pub fn persistent(path: &str, name: &str, cache_size_mb: usize) -> Result<Box<dyn HammersbaldAPI>, Error> {
	std::fs::create_dir_all(path).unwrap();

	let data = DataFile::new(Box::new(CachedFile::new(
		Box::new(AsyncFile::new(Box::new(RolledFile::new(path, name, "bc", file_size())?), "data")?),
		cache_size_mb,
	)?))?;

	let link = DataFile::new(Box::new(CachedFile::new(
		Box::new(AsyncFile::new(Box::new(RolledFile::new(path, name, "bl", file_size())?), "link")?),
		cache_size_mb,
	)?))?;

	let log = LogFile::new(Box::new(RolledFile::new(path, name, "lg", PAGE_SIZE as u64)?));

	let table = TableFile::new(Box::new(CachedFile::new(
		Box::new(AsyncFile::new(Box::new(RolledFile::new(path, name, "tb", file_size())?), "table")?),
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

		let mut db = persistent(path, "test", 1).unwrap();
		let (pref, result) = db.get_keyed(key).unwrap().unwrap();
		assert_eq!(pref, expected_pref);
		assert_eq!(value, result.as_slice());
	}

	#[test]
	fn test_truncate() {
		let path = "testdb/truncate";
		std::fs::remove_dir_all(path).unwrap_or_default();

		{
			let mut db = persistent(path, "test", 1).unwrap();
			db.put_keyed("a".as_bytes(), &[1]).unwrap();
			let pref = db.put_keyed("b".as_bytes(), &[2]).unwrap();
			db.put_keyed("c".as_bytes(), &[3]).unwrap();

			db.batch().unwrap();
			db.truncate(pref).unwrap();
			db.shutdown().unwrap();
		}

		let mut db = persistent(path, "test", 1).unwrap();
		assert_eq!(vec![1], db.get_keyed("a".as_bytes()).unwrap().unwrap().1);
		assert_eq!(None, db.get_keyed("b".as_bytes()).unwrap());
		assert_eq!(None, db.get_keyed("c".as_bytes()).unwrap());

		// Reuse same key
		db.put_keyed("b".as_bytes(), &[4]).unwrap();
		db.batch().unwrap();
		assert_eq!(vec![4], db.get_keyed("b".as_bytes()).unwrap().unwrap().1);
	}
}
