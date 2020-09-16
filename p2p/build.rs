use std::fmt::Write;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader, Lines};
use std::path::Path;
use std::{env, io};

fn main() {
	write_seeds("main");
	write_seeds("test");

	println!("cargo:rerun-if-changed=build.rs");
}

fn write_seeds(network: &str) {
	let out_dir = env::var_os("OUT_DIR").unwrap();

	let ips = read_lines(format!("seeds_{}.txt", network));
	let dest_path = Path::new(&out_dir).join(format!("seeds_{}.rs", network));

	let mut s = String::new();
	writeln!(&mut s, "{}", format!("pub fn seeds_{}() -> Vec<SocketAddr> {{ vec![", network)).unwrap();
	for ip in ips {
		if let Ok(ip) = ip {
			writeln!(&mut s, "\"{}\".parse().unwrap(),", ip).unwrap();
		}
	}
	writeln!(&mut s, "] }}").unwrap();

	fs::write(&dest_path, s).unwrap();

	println!("cargo:rerun-if-changed=seeds_{}.txt", network);
}

fn read_lines<P>(filename: P) -> Lines<BufReader<File>>
where
	P: AsRef<Path>,
{
	let file = File::open(filename).unwrap();
	io::BufReader::new(file).lines()
}
