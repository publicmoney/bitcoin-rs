use crate::pref::PRef;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

// Max number of slots that will fit in the bucket
pub const BUCKET_LENGTH: usize = 291;

#[derive(Clone, Default)]
pub struct Bucket {
	slots: HashMap<u64, PRef, BuildHasherDefault<PreHashed>>,
}

impl Bucket {
	pub fn new() -> Self {
		Bucket {
			slots: HashMap::with_capacity_and_hasher(BUCKET_LENGTH, BuildPreHashed::default()),
		}
	}

	pub fn insert(&mut self, hash: u64, pref: PRef) -> Option<PRef> {
		self.slots.insert(hash, pref)
	}

	pub fn get(&self, hash: &u64) -> Option<&PRef> {
		self.slots.get(hash)
	}

	pub fn remove(&mut self, hash: &u64) -> Option<PRef> {
		self.slots.remove(hash)
	}

	pub fn is_empty(&self) -> bool {
		self.slots.is_empty()
	}

	pub fn len(&self) -> usize {
		self.slots.len()
	}

	pub fn iter(&self) -> impl Iterator<Item = (&u64, &PRef)> {
		self.slots.iter()
	}

	pub fn into_iter(self) -> impl IntoIterator<Item = (u64, PRef)> {
		self.slots.into_iter()
	}
}

// We hash the key outside of the HashMap so this hasher is just a no-op.
#[derive(Default)]
pub struct PreHashed(u64);

impl Hasher for PreHashed {
	fn finish(&self) -> u64 {
		self.0
	}

	fn write(&mut self, _bytes: &[u8]) {
		unreachable!()
	}

	fn write_u64(&mut self, i: u64) {
		self.0 = i
	}
}

pub type BuildPreHashed = BuildHasherDefault<PreHashed>;
