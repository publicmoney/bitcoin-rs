use std::collections::VecDeque;
use std::sync::Mutex;
use time;

/// Speed meter with given items number
#[derive(Debug, Default)]
pub struct AverageSpeedMeter {
	/// Number of items to inspect
	inspect_items: usize,
	/// Number of items currently inspected
	inspected_items: Mutex<VecDeque<f64>>,
	/// Current speed
	speed: Mutex<f64>,
	/// Last timestamp
	last_timestamp: Mutex<Option<f64>>,
}

impl AverageSpeedMeter {
	pub fn with_inspect_items(inspect_items: usize) -> Self {
		assert!(inspect_items > 0);
		AverageSpeedMeter {
			inspect_items,
			inspected_items: Mutex::new(VecDeque::with_capacity(inspect_items)),
			speed: Mutex::new(0_f64),
			last_timestamp: Mutex::new(None),
		}
	}

	pub fn speed(&self) -> f64 {
		let speed = self.speed.lock().unwrap();
		let items_per_second = 1_f64 / *speed;
		if items_per_second.is_normal() {
			items_per_second
		} else {
			0_f64
		}
	}

	pub fn inspected_items_len(&self) -> usize {
		self.inspected_items.lock().unwrap().len()
	}

	pub fn checkpoint(&self) {
		let mut speed = self.speed.lock().unwrap();
		let mut inspected_items = self.inspected_items.lock().unwrap();
		// if inspected_items is already full => remove oldest item from average
		if inspected_items.len() == self.inspect_items {
			let oldest = inspected_items.pop_front().expect("len() is not zero; qed");
			*speed = (self.inspect_items as f64 * *speed - oldest) / (self.inspect_items as f64 - 1_f64);
		}

		// add new item
		let now = time::precise_time_s();
		let mut last_timestamp = self.last_timestamp.lock().unwrap();
		if let Some(last_timestamp) = *last_timestamp {
			let newest = now - last_timestamp;
			*speed = (inspected_items.len() as f64 * *speed + newest) / (inspected_items.len() as f64 + 1_f64);
			inspected_items.push_back(newest);
		}
		*last_timestamp = Some(now);
	}

	pub fn start(&self) {
		*self.last_timestamp.lock().unwrap() = Some(time::precise_time_s());
	}

	pub fn stop(&self) {
		*self.last_timestamp.lock().unwrap() = None;
	}

	#[cfg(test)]
	pub fn set_speed(&self, new_speed: f64) {
		*self.speed.lock().unwrap() = new_speed
	}
}
