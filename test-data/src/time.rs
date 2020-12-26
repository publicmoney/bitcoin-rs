use primitives::time::Time;
use std::cell::Cell;
use std::time::Duration;

#[derive(Default)]
pub struct IncrementalTime {
	counter: Cell<u64>,
}

impl Time for IncrementalTime {
	fn now(&self) -> Duration {
		let c = self.counter.get();
		let result = Duration::from_secs(c);
		self.counter.set(c + 1);
		result
	}
}

#[derive(Default)]
pub struct ZeroTime {}

impl Time for ZeroTime {
	fn now(&self) -> Duration {
		Duration::from_secs(0)
	}
}
