use crate::config::Config;
use clap::ArgMatches;
use import::open_blk_dir;
use storage::SharedStore;
use sync::{create_sync_blocks_writer, Error};

pub async fn import(db: SharedStore, cfg: Config, matches: &ArgMatches<'_>) -> Result<(), String> {
	let blk_path = matches.value_of("PATH").expect("PATH is required in cli.yml; qed");
	let blk_dir = open_blk_dir(blk_path).map_err(|err| format!("Failed to open import directory: {}", err))?;

	let mut writer = create_sync_blocks_writer(db, cfg.consensus, cfg.verification_params);
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

	info!("Finished import of {} blocks", counter);

	Ok(())
}
