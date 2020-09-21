use storage::SharedStore;

pub fn info(db: SharedStore) -> Result<(), String> {
	info!("Getting database statistics. This may take a while.");
	db.info();
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

		store.info();
	}
}
