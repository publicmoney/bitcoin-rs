use crate::app_dir::app_path;
use crate::config::Config;

pub fn stats(cfg: &Config) -> Result<(), String> {
	let db = db::RawDatabase::persistent(&app_path(&cfg.data_dir, "db"), cfg.db_cache).unwrap();
	info!("Getting database statistics. This may take a while.");
	db.stats().unwrap();
	db.shutdown().unwrap();
	info!("Finished stats");
	Ok(())
}
