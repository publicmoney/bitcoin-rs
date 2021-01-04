use crate::app_dir::app_path;
use crate::config::Config;
use clap::ArgMatches;
use import::open_blk_dir;
use std::sync::Arc;
use storage::CanonStore;
use sync::{create_sync_blocks_writer, Error};

pub fn import(cfg: &Config, matches: &ArgMatches) -> Result<(), String> {
	let db =
		Arc::new(db::BlockChainDatabase::persistent(&app_path(&cfg.data_dir, "db"), cfg.db_cache, &cfg.network.genesis_block()).unwrap());
	let blk_path = matches.value_of("PATH").expect("PATH is required in cli.yml; qed");
	let blk_dir = open_blk_dir(blk_path).map_err(|err| format!("Failed to open import directory: {}", err))?;

	let mut writer = create_sync_blocks_writer(db.clone(), cfg.consensus.clone(), cfg.verification_params.clone());
	let mut counter = 0;
	let mut previous_hash = None;
	for blk in blk_dir {
		// TODO: verify magic!
		let blk = blk.map_err(|err| format!("Cannot read block: {:?}. Previous block: {:?}", err, previous_hash))?;
		let blk_hash = blk.block.hash().clone();
		match writer.append_block(blk.block) {
			Ok(_) => {
				counter += 1;
				if counter % 1000 == 0 {
					info!(target: "sync", "Imported {} blocks", counter);
				}
			}
			Err(Error::TooManyOrphanBlocks) => return Err("Too many orphan (unordered) blocks".into()),
			Err(err) => return Err(format!("Cannot append block: {:?}. Block: {}", err, blk_hash)),
		}

		previous_hash = Some(blk_hash);
	}
	db.as_store().shutdown();
	info!("Finished import of {} blocks", counter);

	Ok(())
}
