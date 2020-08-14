use crate::config::Config;
use bitcrypto::SHA256D;
use clap::ArgMatches;
use storage::{BlockRef, SharedStore};

pub async fn rollback(db: SharedStore, cfg: Config, matches: &ArgMatches<'_>) -> Result<(), String> {
	let block_ref = matches.value_of("BLOCK").expect("BLOCK is required in cli.yml; qed");
	let block_ref = if block_ref.len() == 64 {
		BlockRef::Hash(block_ref.parse().map_err(|e| format!("Invalid block number: {}", e))?)
	} else {
		BlockRef::Number(block_ref.parse().map_err(|e| format!("Invalid block hash: {}", e))?)
	};

	let required_block_hash = db
		.block_header(block_ref.clone())
		.ok_or(format!("Block {:?} is unknown", block_ref))?
		.hash;
	let genesis_hash = *cfg.network.genesis_block().hash();

	let mut best_block_hash = db.best_block().hash;
	debug_assert!(best_block_hash != SHA256D::default()); // genesis inserted in init_db

	loop {
		if best_block_hash == required_block_hash {
			info!("Reverted to block {:?}", block_ref);
			return Ok(());
		}

		if best_block_hash == genesis_hash {
			return Err(format!("Failed to revert to block {:?}. Reverted to genesis", block_ref));
		}

		best_block_hash = db.rollback_best().map_err(|e| format!("{:?}", e))?;
	}
}
