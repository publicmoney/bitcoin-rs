use crate::config::Config;

pub fn stats(db_path: &String, cfg: &Config) -> Result<(), String> {
	let db = db::RawDatabase::persistent(db_path, cfg.db_cache).unwrap();
	info!("Getting database statistics. This may take a while.");
	db.stats().unwrap();
	info!("Finished");
	Ok(())
}

#[cfg(test)]
mod tests {
	use db::BlockChainDatabase;
	use storage::Store;

	#[test]
	fn test_info() {
		let store = BlockChainDatabase::init_test_chain(vec![
			test_data::block_h0().into(),
			test_data::block_h1().into(),
			test_data::block_h2().into(),
		]);

		store.stats();
	}
}
