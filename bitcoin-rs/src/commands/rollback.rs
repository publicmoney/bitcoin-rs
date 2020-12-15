use clap::ArgMatches;
use storage::{BlockRef, SharedStore};

pub fn rollback(db: SharedStore, matches: &ArgMatches) -> Result<(), String> {
	let block_ref = matches.value_of("BLOCK").expect("BLOCK is required in cli.yml; qed");
	let block_ref = if block_ref.len() == 64 {
		BlockRef::Hash(block_ref.parse().map_err(|e| format!("Invalid block number: {}", e))?)
	} else {
		BlockRef::Number(block_ref.parse().map_err(|e| format!("Invalid block hash: {}", e))?)
	};

	db.truncate(&block_ref).map_err(|e| format!("{:?}", e))?;
	info!("Rolled back blockchain database to {:?}", block_ref);
	Ok(())
}
