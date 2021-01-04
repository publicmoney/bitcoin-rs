use crate::app_dir::app_path;
use crate::config::Config;
use clap::ArgMatches;
use storage::BlockRef;

pub fn rollback(cfg: &Config, matches: &ArgMatches) -> Result<(), String> {
	let db = db::RawDatabase::persistent(&app_path(&cfg.data_dir, "db"), cfg.db_cache).unwrap();

	let block_ref = matches.value_of("BLOCK").expect("BLOCK is required in cli.yml; qed");
	let block_ref = if block_ref.len() == 64 {
		BlockRef::Hash(block_ref.parse().map_err(|e| format!("Invalid block number: {}", e))?)
	} else {
		BlockRef::Number(block_ref.parse().map_err(|e| format!("Invalid block hash: {}", e))?)
	};

	db.truncate(&block_ref).map_err(|e| format!("{:?}", e))?;
	db.shutdown().unwrap();

	info!("Rolled back blockchain database to {:?}", block_ref);
	Ok(())
}
