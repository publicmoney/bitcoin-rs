use bitcrypto::SHA256D;
use std::collections::{HashSet, VecDeque};
use std::iter::repeat;
use std::ops::Index;

/// Block position
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum HashPosition {
	/// Block is not in the queue
	Missing,
	/// Block is at the front of the queue
	Front,
	/// Block is somewhere inside in the queue
	Inside(u32),
}

/// Ordered queue with O(1) contains() && random access operations cost.
#[derive(Debug, Clone)]
pub struct HashQueue {
	queue: VecDeque<SHA256D>,
	set: HashSet<SHA256D>,
}

/// Chain of linked queues. First queue has index zero.
#[derive(Debug)]
pub struct HashQueueChain {
	chain: Vec<HashQueue>,
}

impl HashQueue {
	pub fn new() -> Self {
		HashQueue {
			queue: VecDeque::new(),
			set: HashSet::new(),
		}
	}

	/// Clears the queue
	pub fn clear(&mut self) {
		self.set.clear();
		self.queue.clear();
	}

	/// Returns len of the given queue.
	pub fn len(&self) -> u32 {
		self.queue.len() as u32
	}

	/// Returns front element from the given queue.
	pub fn front(&self) -> Option<SHA256D> {
		self.queue.front().cloned()
	}

	/// Returns back element from the given queue.
	pub fn back(&self) -> Option<SHA256D> {
		self.queue.back().cloned()
	}

	/// Returns position of the element in the queue
	pub fn position(&self, hash: &SHA256D) -> Option<u32> {
		self.queue
			.iter()
			.enumerate()
			.filter_map(|(pos, h)| if hash == h { Some(pos as u32) } else { None })
			.nth(0)
	}

	/// Returns element at position
	pub fn at(&self, position: u32) -> Option<SHA256D> {
		self.queue.get(position as usize).cloned()
	}

	/// Returns previous-to back element from the given queue.
	pub fn pre_back(&self) -> Option<SHA256D> {
		let queue_len = self.queue.len();
		if queue_len <= 1 {
			return None;
		}
		Some(self.queue[queue_len - 2].clone())
	}

	/// Returns true if queue contains element.
	pub fn contains(&self, hash: &SHA256D) -> bool {
		self.set.contains(hash)
	}

	/// Returns n elements from the front of the queue
	pub fn front_n(&self, n: u32) -> Vec<SHA256D> {
		self.queue.iter().cloned().take(n as usize).collect()
	}

	/// Removes element from the front of the queue.
	pub fn pop_front(&mut self) -> Option<SHA256D> {
		match self.queue.pop_front() {
			Some(hash) => {
				self.set.remove(&hash);
				Some(hash)
			}
			None => None,
		}
	}

	/// Removes n elements from the front of the queue.
	pub fn pop_front_n(&mut self, n: u32) -> Vec<SHA256D> {
		let mut result: Vec<SHA256D> = Vec::new();
		for _ in 0..n {
			match self.pop_front() {
				Some(hash) => result.push(hash),
				None => return result,
			}
		}
		result
	}

	/// Removes element from the back of the queue.
	pub fn pop_back(&mut self) -> Option<SHA256D> {
		match self.queue.pop_back() {
			Some(hash) => {
				self.set.remove(&hash);
				Some(hash)
			}
			None => None,
		}
	}

	/// Adds element to the back of the queue.
	pub fn push_back(&mut self, hash: SHA256D) {
		if !self.set.insert(hash.clone()) {
			panic!("must be checked by caller");
		}
		self.queue.push_back(hash);
	}

	/// Adds elements to the back of the queue.
	pub fn push_back_n(&mut self, hashes: Vec<SHA256D>) {
		for hash in hashes {
			self.push_back(hash);
		}
	}

	/// Removes element from the queue, returning its position.
	pub fn remove(&mut self, hash: &SHA256D) -> HashPosition {
		if !self.set.remove(hash) {
			return HashPosition::Missing;
		}

		if self.queue.front().expect("checked one line above") == hash {
			self.queue.pop_front();
			return HashPosition::Front;
		}

		for i in 0..self.queue.len() {
			if self.queue[i] == *hash {
				self.queue.remove(i);
				return HashPosition::Inside(i as u32);
			}
		}

		// unreachable because hash is not missing, not at the front and not inside
		unreachable!()
	}

	/// Removes all elements from the queue.
	pub fn remove_all(&mut self) -> VecDeque<SHA256D> {
		use std::mem::replace;

		self.set.clear();
		replace(&mut self.queue, VecDeque::new())
	}
}

impl Index<u32> for HashQueue {
	type Output = SHA256D;

	fn index(&self, index: u32) -> &Self::Output {
		&self.queue[index as usize]
	}
}

impl HashQueueChain {
	/// Creates chain with given number of queues.
	pub fn with_number_of_queues(number_of_queues: usize) -> Self {
		assert!(number_of_queues != 0);
		HashQueueChain {
			chain: repeat(HashQueue::new()).take(number_of_queues).collect(),
		}
	}

	/// Returns length of the whole chain.
	pub fn len(&self) -> u32 {
		self.chain.iter().fold(0, |total, chain| total + chain.len())
	}

	/// Returns length of the given queue.
	pub fn len_of(&self, queue_index: usize) -> u32 {
		self.chain[queue_index].len()
	}

	/// Returns element at the given position
	pub fn at(&self, mut index: u32) -> Option<SHA256D> {
		for queue in &self.chain {
			let queue_len = queue.len();
			if index < queue_len {
				return queue.at(index);
			}

			index -= queue_len;
		}

		None
	}

	/// Returns element at the front of the given queue.
	pub fn front_at(&self, queue_index: usize) -> Option<SHA256D> {
		let queue = &self.chain[queue_index];
		queue.front()
	}

	/// Returns element at the front of the given queue.
	pub fn back_at(&self, queue_index: usize) -> Option<SHA256D> {
		let queue = &self.chain[queue_index];
		queue.back()
	}

	/// Returns previous-to back element from the given queue.
	pub fn pre_back_at(&self, chain_index: usize) -> Option<SHA256D> {
		let queue = &self.chain[chain_index];
		queue.pre_back()
	}

	/// Returns the back of the whole chain.
	pub fn back(&self) -> Option<SHA256D> {
		let mut queue_index = self.chain.len() - 1;
		loop {
			let queue = &self.chain[queue_index];
			let queue_back = queue.back();
			if queue_back.is_some() {
				return queue_back;
			}

			queue_index -= 1;
			if queue_index == 0 {
				return None;
			}
		}
	}

	/// Checks if hash is contained in given queue.
	#[cfg(test)]
	pub fn is_contained_in(&self, queue_index: usize, hash: &SHA256D) -> bool {
		self.chain[queue_index].contains(hash)
	}

	/// Returns the index of queue, hash is contained in.
	pub fn contains_in(&self, hash: &SHA256D) -> Option<usize> {
		for i in 0..self.chain.len() {
			if self.chain[i].contains(hash) {
				return Some(i);
			}
		}
		None
	}

	/// Returns n elements from the front of the given queue
	pub fn front_n_at(&self, queue_index: usize, n: u32) -> Vec<SHA256D> {
		self.chain[queue_index].front_n(n)
	}

	/// Remove a number of hashes from the front of the given queue.
	pub fn pop_front_n_at(&mut self, queue_index: usize, n: u32) -> Vec<SHA256D> {
		self.chain[queue_index].pop_front_n(n)
	}

	/// Push hash onto the back of the given queue.
	pub fn push_back_at(&mut self, queue_index: usize, hash: SHA256D) {
		self.chain[queue_index].push_back(hash)
	}

	/// Push a number of hashes onto the back of the given queue.
	pub fn push_back_n_at(&mut self, queue_index: usize, hashes: Vec<SHA256D>) {
		self.chain[queue_index].push_back_n(hashes)
	}

	/// Remove hash from given queue.
	pub fn remove_at(&mut self, queue_index: usize, hash: &SHA256D) -> HashPosition {
		self.chain[queue_index].remove(hash)
	}

	/// Remove all items from given queue.
	pub fn remove_all_at(&mut self, queue_index: usize) -> VecDeque<SHA256D> {
		self.chain[queue_index].remove_all()
	}
}

impl Index<u32> for HashQueueChain {
	type Output = SHA256D;

	fn index(&self, mut index: u32) -> &Self::Output {
		for queue in &self.chain {
			let queue_len = queue.len();
			if index < queue_len {
				return &queue[index];
			}

			index -= queue_len;
		}

		panic!("invalid index");
	}
}

#[cfg(test)]
mod tests {
	use super::{HashPosition, HashQueue, HashQueueChain};
	use bitcrypto::{FromStr, SHA256D};

	#[test]
	fn hash_queue_empty() {
		let mut queue = HashQueue::new();
		assert_eq!(queue.len(), 0);
		assert_eq!(queue.front(), None);
		assert_eq!(queue.back(), None);
		assert_eq!(queue.pre_back(), None);
		assert_eq!(
			queue.contains(&SHA256D::from_str("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f").unwrap()),
			false
		);
		assert_eq!(queue.pop_front(), None);
		assert_eq!(queue.pop_front_n(100), Vec::<SHA256D>::new());
		assert_eq!(
			queue.remove(&SHA256D::from_str("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f").unwrap()),
			HashPosition::Missing
		);
	}

	#[test]
	fn hash_queue_chain_empty() {
		let mut chain = HashQueueChain::with_number_of_queues(3);
		assert_eq!(chain.len(), 0);
		assert_eq!(chain.len_of(0), 0);
		assert_eq!(chain.front_at(0), None);
		assert_eq!(chain.back_at(0), None);
		assert_eq!(chain.pre_back_at(0), None);
		assert_eq!(chain.back(), None);
		assert_eq!(
			chain.is_contained_in(
				0,
				&SHA256D::from_str("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f").unwrap()
			),
			false
		);
		assert_eq!(
			chain.contains_in(&SHA256D::from_str("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f").unwrap()),
			None
		);
		assert_eq!(chain.pop_front_n_at(0, 100), Vec::<SHA256D>::new());
		assert_eq!(
			chain.remove_at(
				0,
				&SHA256D::from_str("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f").unwrap()
			),
			HashPosition::Missing
		);
	}

	#[test]
	fn hash_queue_chain_not_empty() {
		let mut chain = HashQueueChain::with_number_of_queues(4);
		chain.push_back_n_at(
			0,
			vec![
				SHA256D::default(),
				SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
				SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap(),
			],
		);
		chain.push_back_n_at(
			1,
			vec![
				SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000003").unwrap(),
				SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000004").unwrap(),
			],
		);
		chain.push_back_n_at(
			2,
			vec![SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000005").unwrap()],
		);

		assert_eq!(chain.len(), 6);
		assert_eq!(chain.len_of(0), 3);
		assert_eq!(chain.len_of(1), 2);
		assert_eq!(chain.len_of(2), 1);
		assert_eq!(chain.len_of(3), 0);
		assert_eq!(chain.front_at(0), Some(SHA256D::default()));
		assert_eq!(
			chain.front_at(1),
			Some(SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000003").unwrap())
		);
		assert_eq!(
			chain.front_at(2),
			Some(SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000005").unwrap())
		);
		assert_eq!(chain.front_at(3), None);
		assert_eq!(
			chain.back_at(0),
			Some(SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap())
		);
		assert_eq!(
			chain.back_at(1),
			Some(SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000004").unwrap())
		);
		assert_eq!(
			chain.back_at(2),
			Some(SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000005").unwrap())
		);
		assert_eq!(chain.back_at(3), None);
		assert_eq!(
			chain.pre_back_at(0),
			Some(SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap())
		);
		assert_eq!(
			chain.pre_back_at(1),
			Some(SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000003").unwrap())
		);
		assert_eq!(chain.pre_back_at(2), None);
		assert_eq!(chain.pre_back_at(3), None);
		assert_eq!(
			chain.back(),
			Some(SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000005").unwrap())
		);
		assert_eq!(
			chain.is_contained_in(
				0,
				&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap()
			),
			true
		);
		assert_eq!(
			chain.is_contained_in(
				1,
				&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap()
			),
			false
		);
		assert_eq!(
			chain.is_contained_in(
				2,
				&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap()
			),
			false
		);
		assert_eq!(
			chain.is_contained_in(
				3,
				&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap()
			),
			false
		);
		assert_eq!(
			chain.contains_in(&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap()),
			Some(0)
		);
		assert_eq!(
			chain.contains_in(&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000005").unwrap()),
			Some(2)
		);
		assert_eq!(
			chain.contains_in(&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000009").unwrap()),
			None
		);
	}

	#[test]
	fn hash_queue_front_n() {
		let mut queue = HashQueue::new();
		queue.push_back_n(vec![
			SHA256D::default(),
			SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
		]);
		assert_eq!(
			queue.front_n(3),
			vec![
				SHA256D::default(),
				SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap()
			]
		);
		assert_eq!(
			queue.front_n(3),
			vec![
				SHA256D::default(),
				SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap()
			]
		);
		assert_eq!(
			queue.pop_front_n(3),
			vec![
				SHA256D::default(),
				SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap()
			]
		);
		assert_eq!(queue.pop_front_n(3), Vec::<SHA256D>::new());
	}
}
