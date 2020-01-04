extern crate byteorder;
extern crate chain;
extern crate db;
extern crate network;
extern crate primitives;
extern crate storage;
extern crate test_data;
extern crate time;
extern crate verification;

mod database;
mod verifier;

use std::io::Write;
use std::str;
use time::{Duration, Instant};

#[derive(Default)]
pub struct Benchmark {
	start: Option<Instant>,
	end: Option<Instant>,
	samples: Option<usize>,
}

impl Benchmark {
	pub fn start(&mut self) {
		self.start = Some(Instant::now());
	}

	pub fn stop(&mut self) {
		self.end = Some(Instant::now());
	}

	pub fn evaluate(&self) -> Duration {
		self.end.expect("benchmark never started") - self.start.expect("benchmarch never ended")
	}

	pub fn samples(&mut self, samples: usize) {
		self.samples = Some(samples);
	}
}

fn decimal_mark(s: String) -> String {
	let bytes: Vec<_> = s.bytes().rev().collect();
	let chunks: Vec<_> = bytes.chunks(3).map(|chunk| str::from_utf8(chunk).unwrap()).collect();
	let result: Vec<_> = chunks.join(",").bytes().rev().collect();
	String::from_utf8(result).unwrap()
}

fn run_benchmark<F>(name: &str, f: F)
where
	F: FnOnce(&mut Benchmark),
{
	print!("{}: ", name);
	::std::io::stdout().flush().unwrap();

	let mut benchmark = Benchmark::default();
	f(&mut benchmark);
	if let Some(samples) = benchmark.samples {
		println!(
			"{} ns/sample",
			decimal_mark(format!("{}", benchmark.evaluate().whole_nanoseconds() / samples as i128)),
		);
	} else {
		println!("{} ns", decimal_mark(format!("{}", benchmark.evaluate().whole_nanoseconds())));
	}
}

macro_rules! benchmark {
	($t:expr) => {
		run_benchmark(stringify!($t), $t);
	};
}

fn main() {
	benchmark!(database::fetch);
	benchmark!(database::write);
	benchmark!(database::reorg_short);
	benchmark!(database::write_heavy);
	benchmark!(verifier::main);
}
