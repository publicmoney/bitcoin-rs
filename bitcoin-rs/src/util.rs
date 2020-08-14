use crate::config::Config;
use crate::APP_INFO;
use app_dirs::{app_dir, AppDataType};
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::sync::Arc;
use storage::CanonStore;

pub fn db_path(data_dir: &Option<String>) -> String {
	let db_path = match *data_dir {
		Some(ref data_dir) => custom_path(&data_dir, "db"),
		None => app_dir(AppDataType::UserData, &APP_INFO, "db").expect("Failed to get app dir"),
	};
	db_path.to_str().unwrap().to_string()
}

pub fn open_db(cfg: &Config) -> Result<storage::SharedStore, String> {
	let db_path = db_path(&cfg.data_dir);
	let db = Arc::new(db::BlockChainDatabase::persistent(db_path, cfg.db_cache).unwrap());

	let genesis_block = cfg.network.genesis_block();

	match db.as_store().block_hash(0) {
		Some(ref db_genesis_block_hash) if db_genesis_block_hash != genesis_block.hash() => {
			Err("Trying to open database with incompatible genesis block".into())
		}
		Some(_) => Ok(db),
		None => {
			info!("Initialising database with genesis block: {:?}", genesis_block.hash());
			let hash = genesis_block.hash().clone();
			db.insert(genesis_block).expect("Failed to insert genesis block to the database");
			db.canonize(&hash).expect("Failed to canonize genesis block");
			Ok(db)
		}
	}
}

pub fn node_table_path(data_dir: &Option<String>) -> PathBuf {
	let mut node_table = match data_dir {
		Some(ref data_dir) => custom_path(&data_dir, "p2p"),
		None => app_dir(AppDataType::UserData, &APP_INFO, "p2p").expect("Failed to get app dir"),
	};
	node_table.push("nodes.csv");
	node_table
}

fn custom_path(data_dir: &str, sub_dir: &str) -> PathBuf {
	let mut path = PathBuf::from(data_dir);
	path.push(sub_dir);
	create_dir_all(&path).expect("Failed to get app dir");
	path
}
