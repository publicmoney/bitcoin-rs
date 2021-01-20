use crate::pref::PRef;
use std::collections::HashMap;
use std::convert::TryInto;
use std::hash::{BuildHasherDefault, Hasher};

// Max number of slots to be stored in the bucket
pub const BUCKET_LENGTH: usize = 408;

#[derive(Clone, Default)]
pub struct Bucket {
	slots: HashMap<u32, PRef, BuildHasherDefault<PreHashed>>,
}

impl Bucket {
	pub fn new() -> Self {
		Bucket {
			slots: HashMap::with_capacity_and_hasher(BUCKET_LENGTH, BuildPreHashed::default()),
		}
	}

	pub fn insert(&mut self, hash: u32, pref: PRef) -> Option<PRef> {
		self.slots.insert(hash, pref)
	}

	pub fn get(&self, hash: &u32) -> Option<&PRef> {
		self.slots.get(hash)
	}

	pub fn remove(&mut self, hash: &u32) -> Option<PRef> {
		self.slots.remove(hash)
	}

	pub fn is_empty(&self) -> bool {
		self.slots.is_empty()
	}

	pub fn len(&self) -> usize {
		self.slots.len()
	}

	pub fn iter(&self) -> impl Iterator<Item = (&u32, &PRef)> {
		self.slots.iter()
	}

	pub fn into_iter(self) -> impl IntoIterator<Item = (u32, PRef)> {
		self.slots.into_iter()
	}
}

// We hash the key outside of the HashMap so this hasher is just a no-op.
#[derive(Default)]
pub struct PreHashed(u32);

impl Hasher for PreHashed {
	fn finish(&self) -> u64 {
		self.0 as u64
	}

	fn write(&mut self, bytes: &[u8]) {
		self.0 = u32::from_ne_bytes(bytes.try_into().unwrap())
	}

	fn write_u32(&mut self, i: u32) {
		self.0 = i
	}
}

pub type BuildPreHashed = BuildHasherDefault<PreHashed>;
