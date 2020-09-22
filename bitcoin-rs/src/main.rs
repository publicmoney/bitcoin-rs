//! bitcoin-rs client.

#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;
extern crate app_dirs;
extern crate env_logger;
extern crate libc;

mod commands;
mod config;
mod rpc;
mod rpc_apis;
mod util;

use crate::config::USER_AGENT;
use crate::util::open_db;
use app_dirs::AppInfo;
use tokio::runtime;
use tokio::runtime::Runtime;
use tokio::time::Duration;

#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

pub const APP_INFO: AppInfo = AppInfo {
	name: USER_AGENT,
	author: "publicmoney",
};
pub const LOG_INFO: &'static str = "info";

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

	if !cfg.quiet {
		if cfg!(windows) {
			logs::init(LOG_INFO, logs::DateLogFormatter);
		} else {
			logs::init(LOG_INFO, logs::DateAndColorLogFormatter);
		}
	} else {
		env_logger::init();
	}

	let db = open_db(&cfg)?;

	let mut threaded_rt: Runtime = runtime::Builder::new()
		.threaded_scheduler()
		.enable_io()
		.enable_time()
		.build()
		.expect("Failure starting Tokio runtime");

	match matches.subcommand() {
		("import", Some(import_matches)) => commands::import(db.clone(), cfg, import_matches),
		("info", Some(_)) => commands::info(db.clone()),
		("rollback", Some(rollback_matches)) => commands::rollback(db.clone(), cfg, rollback_matches),
		("verify", Some(_)) => commands::verify(db.clone(), cfg),
		_ => {
			let result = commands::start(&threaded_rt, db.clone(), cfg);
			threaded_rt.block_on(tokio::signal::ctrl_c()).expect("Runtime error");
			result
		}
	}?;

	info!("Shutting down, please wait...");
	threaded_rt.shutdown_timeout(Duration::from_secs(3));
	db.shutdown();
	Ok(())
}
