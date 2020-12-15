//! bitcoin-rs client.

#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;
extern crate app_dirs;
extern crate libc;

mod app_dir;
mod block_notifier;
mod commands;
mod config;
mod logger;
mod rpc;
mod rpc_apis;

use crate::app_dir::app_path;
use std::sync::Arc;
use storage::CanonStore;
use tokio::runtime;
use tokio::runtime::Runtime;
use tokio::time::Duration;

#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn main() {
	// Always print backtrace on panic.
	::std::env::set_var("RUST_BACKTRACE", "1");

	if let Err(err) = run() {
		println!("{}", err);
	}
}

fn run() -> Result<(), String> {
	let yaml = load_yaml!("cli.yml");
	let matches = clap::App::from_yaml(yaml).get_matches();
	let cfg = config::parse(&matches)?;

	logger::setup_log(&cfg)?;

	let db_path = app_path(&cfg.data_dir, "db");
	let db = Arc::new(db::BlockChainDatabase::persistent(&db_path, cfg.db_cache, &cfg.network.genesis_block()).unwrap());

	let mut threaded_rt: Runtime = runtime::Builder::new()
		.threaded_scheduler()
		.enable_io()
		.enable_time()
		.build()
		.expect("Failure starting Tokio runtime");

	match matches.subcommand() {
		("import", Some(import_matches)) => commands::import(db.clone(), cfg, import_matches),
		("info", Some(_)) => commands::stats(db.clone()),
		("rollback", Some(rollback_matches)) => commands::rollback(db.clone(), rollback_matches),
		("verify", Some(_)) => commands::verify(db.clone(), cfg),
		_ => {
			let result = commands::start(&threaded_rt, db.clone(), cfg);
			threaded_rt.block_on(tokio::signal::ctrl_c()).expect("Runtime error");
			result
		}
	}?;

	info!("Shutting down, please wait...");
	threaded_rt.shutdown_timeout(Duration::from_secs(3));
	db.as_store().shutdown();
	Ok(())
}
