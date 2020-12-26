use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub trait Time {
	// The duration since the UNIX epoch.
	fn now(&self) -> Duration;
}

#[derive(Default, Debug)]
pub struct RealTime;

impl Time for RealTime {
	fn now(&self) -> Duration {
		SystemTime::now().duration_since(UNIX_EPOCH).unwrap()
	}
}
