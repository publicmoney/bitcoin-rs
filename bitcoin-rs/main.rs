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
mod seednodes;
mod util;

use app_dirs::AppInfo;

pub const USER_AGENT: &'static str = env!("CARGO_PKG_NAME");
pub const USER_AGENT_VERSION: &'static str = env!("CARGO_PKG_VERSION");

pub const APP_INFO: AppInfo = AppInfo {
	name: USER_AGENT,
	author: "publicmoney",
};
pub const PROTOCOL_VERSION: u32 = 70_014;
pub const PROTOCOL_MINIMUM: u32 = 70_001;
pub const REGTEST_USER_AGENT: &'static str = "/Satoshi:0.12.1/";
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

	match matches.subcommand() {
		("import", Some(import_matches)) => commands::import(cfg, import_matches),
		("rollback", Some(rollback_matches)) => commands::rollback(cfg, rollback_matches),
		_ => commands::start(cfg),
	}
}
