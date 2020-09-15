use crate::config::Config;
use std::time::SystemTime;
use storage::{BlockRef, SharedStore};
use verification::ChainVerifier;

pub fn verify(db: SharedStore, cfg: Config) -> Result<(), String> {
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

	info!("Chain verification from genesis to block {} completed successfully", best.number);

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::verify;
	use crate::config::Config;
	use db::BlockChainDatabase;
	use std::sync::Arc;

	#[test]
	fn test_verify() {
		let store = BlockChainDatabase::init_test_chain(vec![
			test_data::block_h0().into(),
			test_data::block_h1().into(),
			test_data::block_h2().into(),
		]);

		assert_eq!(Ok(()), verify(Arc::new(store), Config::default()))
	}
}
