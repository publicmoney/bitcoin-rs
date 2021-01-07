#![forbid(unsafe_code)]

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

#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn main() {
	// Always print backtrace on panic.
	::std::env::set_var("RUST_BACKTRACE", "1");

	if let Err(err) = run() {
		println!("{}", err);
	}
	info!("Shutdown")
}

fn run() -> Result<(), String> {
	let yaml = load_yaml!("cli.yml");
	let matches = clap::App::from_yaml(yaml).get_matches();
	let cfg = config::parse(&matches)?;

	logger::setup_log(&cfg)?;

	match matches.subcommand() {
		("rollback", Some(rollback_matches)) => commands::rollback(&cfg, rollback_matches),
		("stats", Some(_)) => commands::stats(&cfg),
		("import", Some(import_matches)) => commands::import(&cfg, import_matches),
		("verify", Some(_)) => commands::verify(&cfg),
		_ => commands::start(cfg),
	}
}
