use crate::app_dir::app_path;
use crate::config::Config;
use std::time::SystemTime;
use storage::{BlockProvider, BlockRef, CanonStore};
use verification::ChainVerifier;

pub fn verify(cfg: &Config) -> Result<(), String> {
	let db = db::BlockChainDatabase::persistent(&app_path(&cfg.data_dir, "db"), cfg.db_cache, &cfg.network.genesis_block()).unwrap();
	let genesis_hash = *cfg.network.genesis_block().hash();
	match db.block(BlockRef::Number(0)) {
		Some(genesis_block) => {
			if genesis_block.header.hash != genesis_hash {
				return Err(format!(
					"Genesis block mismatch. Expected {}, found on disk: {}",
					genesis_hash,
					genesis_block.hash()
				));
			}
		}
		None => return Err("Genesis block not found".to_string()),
	};

	let best = db.best_block();
	info!("Verifying from block 0 to {}", best.number);

	for block_number in 0..best.number {
		match db.block(BlockRef::Number(block_number)) {
			Some(block) => {
				match ChainVerifier::new(
					&block,
					cfg.network,
					SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() as u32,
				)
				.check()
				{
					Ok(()) => (),
					Err(e) => return Err(format!("Error verifying block {}: {}", block_number, e.to_string())),
				}
			}
			None => return Err(format!("Missing block {}", block_number)),
		}

		if block_number % 1000 == 0 {
			info!("Verified {} blocks", block_number);
		}
	}

	db.as_store().shutdown();
	info!("Chain verification from genesis to block {} completed successfully", best.number);

	Ok(())
}
