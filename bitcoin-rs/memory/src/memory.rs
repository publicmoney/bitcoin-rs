extern crate jemalloc_ctl;

use jemalloc_ctl::{epoch, stats};
use std::time::{Duration, Instant};

pub struct Memory {
	start_time: Instant,
}

impl Memory {
	pub fn new() -> Self {
		Memory {
			start_time: Instant::now(),
		}
	}
}

impl Memory {
	pub fn get_memory_usage(&self) -> Statistics {
		epoch::advance().unwrap();
		let allocated = stats::allocated::read().unwrap();
		let active = stats::active::read().unwrap();
		let resident = stats::resident::read().unwrap();
		let retained = stats::allocated::read().unwrap();
		Statistics {
			allocated,
			active,
			resident,
			retained,
		}
	}

	pub fn uptime(&self) -> Duration {
		self.start_time.elapsed()
	}
}

pub struct Statistics {
	pub allocated: usize,
	pub active: usize,
	pub resident: usize,
	pub retained: usize,
}
