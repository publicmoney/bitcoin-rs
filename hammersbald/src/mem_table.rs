use crate::data_file::{DataFile, EnvelopeIterator};
use crate::error::Error;
use crate::format::{Data, Envelope, IndexedData, Link, Payload};
use crate::log_file::LogFile;
use crate::page::Page;
use crate::paged_file::PagedFile;
use crate::pref::PRef;
use crate::table_file::{TableFile, BUCKETS_FIRST_PAGE, BUCKETS_PER_PAGE, BUCKET_SIZE, FIRST_PAGE_HEAD};

use bitcoin_hashes::siphash24;
use rand::{thread_rng, RngCore};

use lru::LruCache;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::fmt;

pub const BUCKET_FILL_TARGET: usize = 64;
const INIT_BUCKETS: usize = 512;
const INIT_LOGMOD: usize = 8;
const BUCKET_CACHE_MAX_SIZE: usize = 100000;

pub struct MemTable {
	step: usize,
	forget: usize,
	log_mod: u32,
	sip0: u64,
	sip1: u64,
	link_prefs: Vec<PRef>,
	buckets: RwLock<LruCache<usize, Bucket>>,
	dirty: Dirty,
	log_file: LogFile,
	data_file: DataFile,
	table_file: TableFile,
	link_file: DataFile,
}

impl MemTable {
	pub fn new(log_file: LogFile, table_file: TableFile, data_file: DataFile, link_file: DataFile) -> MemTable {
		let mut rng = thread_rng();

		MemTable {
			log_mod: INIT_LOGMOD as u32,
			step: 0,
			forget: 0,
			sip0: rng.next_u64(),
			sip1: rng.next_u64(),
			link_prefs: vec![PRef::invalid(); INIT_BUCKETS],
			buckets: RwLock::new(LruCache::unbounded()),
			dirty: Dirty::new(INIT_BUCKETS),
			log_file,
			table_file,
			data_file,
			link_file,
		}
	}

	pub fn params(&self) -> (usize, u32, usize, u64, u64, u64, u64, u64) {
		(
			self.step,
			self.log_mod,
			self.link_prefs.len(),
			self.table_file.len().unwrap(),
			self.data_file.len().unwrap(),
			self.link_file.len().unwrap(),
			self.sip0,
			self.sip1,
		)
	}

	/// end current batch and start a new batch
	pub fn batch(&mut self) -> Result<(), Error> {
		self.flush()?;

		self.table_file.flush()?;
		self.table_file.sync()?;
		let table_len = self.table_file.len()?;

		self.link_file.flush()?;
		self.link_file.sync()?;
		let link_len = self.link_file.len()?;

		self.data_file.flush()?;
		self.data_file.sync()?;
		let data_len = self.data_file.len()?;

		self.log_file.init(data_len, table_len, link_len)?;
		self.log_file.flush()?;
		self.log_file.sync()?;

		Ok(())
	}

	/// stop background writer
	pub fn shutdown(&mut self) -> Result<(), Error> {
		self.data_file.shutdown()?;
		self.link_file.shutdown()?;
		self.table_file.shutdown()?;
		self.log_file.shutdown()
	}

	pub fn recover(&mut self) -> Result<(), Error> {
		let (data_len, table_len, link_len) = self.log_file.recover()?;
		self.data_file.truncate(data_len)?;
		self.table_file.truncate(table_len)?;
		self.link_file.truncate(link_len)?;
		Ok(())
	}

	pub fn load(&mut self) -> Result<(), Error> {
		if let Some(first) = self.table_file.read_page(PRef::from(0))? {
			let n_buckets = first.read_pref(0).as_u64() as u32;
			self.link_prefs = vec![PRef::invalid(); n_buckets as usize];
			self.buckets = RwLock::new(LruCache::unbounded());
			self.dirty = Dirty::new(n_buckets as usize);
			self.step = first.read_pref(6).as_u64() as usize;
			self.log_mod = (32 - n_buckets.leading_zeros()) as u32 - 2;
			self.sip0 = first.read_u64(12);
			self.sip1 = first.read_u64(20);
		}

		for (i, link) in self.table_file.iter().enumerate() {
			if i < self.link_prefs.len() {
				self.link_prefs[i] = link;
			} else {
				break;
			}
		}

		Ok(())
	}

	fn resolve_bucket(&mut self, bucket_number: usize) -> Result<(), Error> {
		let mut to_flush = HashMap::new();
		{
			let mut buckets = self.buckets.write();
			if let Some(pref) = self.link_prefs.get(bucket_number) {
				if pref.is_valid() && buckets.peek(&bucket_number).is_none() {
					if let Ok(env) = self.link_file.get_envelope(*pref) {
						if env.len() > 0 {
							if let Ok(Payload::Link(link)) = env.payload() {
								let bucket = Bucket { slots: link.slots() };
								buckets.put(bucket_number, bucket);
							}
						}
					}
				}
			}
			if buckets.peek(&bucket_number).is_none() {
				buckets.put(bucket_number, Bucket::default());
			}
			while buckets.len() > BUCKET_CACHE_MAX_SIZE {
				if let Some((num, bucket)) = buckets.pop_lru() {
					if self.dirty.get(num) {
						to_flush.insert(num, bucket);
					}
				}
			}
		}
		for (num, bucket) in to_flush {
			self.flush_bucket(num, Some(&bucket))?;
		}
		Ok(())
	}

	fn flush(&mut self) -> Result<(), Error> {
		{
			// first page
			let fp = PRef::from(0);
			let mut page = self.table_file.read_page(fp)?.unwrap_or(Self::invalid_offsets_page(fp));
			page.write_pref(0, PRef::from(self.link_prefs.len() as u64));
			page.write_pref(6, PRef::from(self.step as u64));
			page.write_u64(12, self.sip0);
			page.write_u64(20, self.sip1);
			self.table_file.update_page(page)?;
		}
		if self.dirty.is_dirty() {
			let dirty_iterator = DirtyIterator::new(&self.dirty);
			let dirty: Vec<usize> = dirty_iterator.enumerate().filter(|a| a.1).map(|a| a.0).collect();
			for num in dirty {
				self.flush_bucket(num, None)?;
			}
		}
		Ok(())
	}

	fn flush_bucket(&mut self, bucket_number: usize, bucket: Option<&Bucket>) -> Result<(), Error> {
		let buckets = self.buckets.read();
		if let Some(bucket) = bucket.or(buckets.peek(&bucket_number)) {
			let link_pref = self
				.link_prefs
				.get(bucket_number)
				.cloned()
				.ok_or(Error::Corrupted(format!("Bucket links {} not found", bucket_number)))?;
			let bucket_pref = TableFile::table_offset(bucket_number);
			let mut page = self
				.table_file
				.read_page(bucket_pref.this_page())?
				.unwrap_or_else(|| Self::invalid_offsets_page(bucket_pref.this_page()));

			let link = if !bucket.slots.is_empty() {
				let links = Link::from_slots(&bucket.slots);
				let payload = Link::deserialize(links.as_slice()).to_payload();
				if link_pref == PRef::invalid() {
					self.link_file.append(payload)?
				} else {
					self.link_file.update(link_pref, payload)?;
					link_pref
				}
			} else {
				PRef::invalid()
			};
			self.link_prefs[bucket_number] = link;
			page.write_pref(bucket_pref.in_page_pos(), link);
			self.table_file.update_page(page)?;
		}
		self.dirty.unset(bucket_number);
		Ok(())
	}

	pub fn invalid_offsets_page(pos: PRef) -> Page {
		let mut page = Page::new_page_with_position(pos);
		if pos.as_u64() == 0 {
			for o in 0..BUCKETS_FIRST_PAGE {
				page.write_pref(FIRST_PAGE_HEAD + o * BUCKET_SIZE, PRef::invalid());
			}
		} else {
			for o in 0..BUCKETS_PER_PAGE {
				page.write_pref(o * BUCKET_SIZE, PRef::invalid());
			}
		}
		page
	}

	pub fn buckets(&mut self) -> BucketIterator {
		BucketIterator { file: self, n: 0 }
	}

	pub fn data_envelopes(&self) -> EnvelopeIterator {
		self.data_file.envelopes()
	}

	pub fn link_envelopes(&self) -> EnvelopeIterator {
		self.link_file.envelopes()
	}

	pub fn append_data(&mut self, key: &[u8], data: &[u8]) -> Result<PRef, Error> {
		self.data_file.append(IndexedData::new(key, Data::new(data)).to_payload())
	}

	pub fn append_referred(&mut self, data: &[u8]) -> Result<PRef, Error> {
		self.data_file.append(Data::new(data).into_payload())
	}

	pub fn get_envelope(&self, pref: PRef) -> Result<Envelope, Error> {
		self.data_file.get_envelope(pref)
	}

	pub fn set(&mut self, pref: PRef, data: &[u8]) -> Result<PRef, Error> {
		self.data_file.set_data(pref, data)
	}

	pub fn put(&mut self, key: &[u8], data_offset: PRef) -> Result<(), Error> {
		let hash = self.hash(key);
		let bucket = self.bucket_for_hash(hash);

		self.store_to_bucket(bucket, hash, data_offset)?;

		if self.forget == 0 {
			if hash % BUCKET_FILL_TARGET as u32 == 0 && self.step < (1 << 31) {
				if self.step < (1 << self.log_mod) {
					let step = self.step;
					self.rehash_bucket(step)?;
				}

				self.step += 1;
				if self.step > (1 << (self.log_mod + 1)) {
					self.log_mod += 1;
					self.step = 0;
				}

				self.link_prefs.push(PRef::invalid());
				self.dirty.append();
			}
		} else {
			self.forget -= 1;
		}
		Ok(())
	}

	pub fn forget(&mut self, key: &[u8]) -> Result<(), Error> {
		let hash = self.hash(key);
		let bucket = self.bucket_for_hash(hash);
		if self.remove_duplicate(key, hash, bucket)? {
			self.forget += 1;
		}
		Ok(())
	}

	fn remove_duplicate(&mut self, key: &[u8], hash: u32, bucket_number: usize) -> Result<bool, Error> {
		let mut remove = None;
		self.resolve_bucket(bucket_number)?;

		if let Some(bucket) = self.buckets.write().get_mut(&bucket_number) {
			for (n, (_, pref)) in bucket.slots.iter().enumerate().filter(|s| (s.1).0 == hash) {
				let envelope = self.data_file.get_envelope(*pref)?;
				if let Payload::Indexed(indexed) = envelope.payload()? {
					if indexed.key == key {
						remove = Some(n);
						break;
					}
				}
			}
			if let Some(r) = remove {
				bucket.slots.remove(r);
			}
		}
		if remove.is_some() {
			self.dirty.set(bucket_number);
		}
		Ok(remove.is_some())
	}

	fn store_to_bucket(&mut self, bucket_number: usize, hash: u32, pref: PRef) -> Result<(), Error> {
		self.resolve_bucket(bucket_number)?;

		if let Some(bucket) = self.buckets.write().get_mut(&bucket_number) {
			bucket.slots.push((hash, pref));
		} else {
			return Err(Error::Corrupted(
				format!("memtable does not have the bucket {}", bucket_number).to_string(),
			));
		}
		self.dirty.set(bucket_number);
		Ok(())
	}

	fn rehash_bucket(&mut self, bucket_number: usize) -> Result<(), Error> {
		let mut rewrite = false;
		let mut new_bucket_store = Bucket::default();
		let mut moves = HashMap::new();
		self.resolve_bucket(bucket_number)?;

		if let Some(b) = self.buckets.write().get(&bucket_number) {
			for (hash, pref) in b.slots.iter() {
				let new_bucket = (hash & (!0u32 >> (32 - self.log_mod - 1))) as usize; // hash % 2^(log_mod + 1)
				if new_bucket != bucket_number {
					moves.entry(new_bucket).or_insert(Vec::new()).push((*hash, *pref));
					rewrite = true;
				} else {
					new_bucket_store.slots.push((*hash, *pref));
				}
			}
		} else {
			return Err(Error::Corrupted(format!("does not have bucket {} for rehash", bucket_number)));
		}

		if rewrite {
			for (bucket, added) in moves {
				for (hash, pref) in added {
					self.store_to_bucket(bucket, hash, pref)?;
				}
			}
			self.link_prefs[bucket_number] = PRef::invalid();
			self.buckets.write().put(bucket_number, new_bucket_store);
			self.dirty.set(bucket_number);
		}
		Ok(())
	}

	pub fn update_key(&mut self, key: &[u8], pref: PRef) -> Result<(), Error> {
		let hash = self.hash(key);
		let bucket_number = self.bucket_for_hash(hash);
		self.resolve_bucket(bucket_number)?;
		if let Some(bucket) = self.buckets.write().get_mut(&bucket_number) {
			for (h, data) in bucket.slots.iter_mut() {
				if *h == hash {
					*data = pref
				}
			}
		}
		self.dirty.set(bucket_number);
		Ok(())
	}

	// get the data last associated with the key
	pub fn get(&mut self, key: &[u8]) -> Result<Option<(PRef, Vec<u8>)>, Error> {
		let hash = self.hash(key);
		let bucket_number = self.bucket_for_hash(hash);
		self.resolve_bucket(bucket_number)?;
		if let Some(bucket) = self.buckets.write().get(&bucket_number) {
			for (h, data) in bucket.slots.iter() {
				if *h == hash {
					// If the database has been truncated then there may be keys that do not have data anymore.
					if data.as_u64() > self.data_file.len()? {
						return Ok(None);
					}
					let envelope = self.data_file.get_envelope(*data)?;
					if envelope.len() == 0 {
						return Ok(None);
					}
					if let Payload::Indexed(indexed) = envelope.payload()? {
						if indexed.key == key {
							return Ok(Some((*data, indexed.data.data.to_vec())));
						}
					} else {
						return Err(Error::Corrupted("pref should point to indexed data".to_string()));
					}
				}
			}
		} else {
			return Err(Error::Corrupted(format!("bucket {} should exist", bucket_number)));
		}
		Ok(None)
	}

	pub fn truncate(&mut self, pref: PRef) -> Result<(), Error> {
		self.data_file.truncate(pref.as_u64())?;

		let mut to_update = vec![];
		let mut highest_used_bucket = 0;
		for (bucket_number, (link_pref, bucket)) in self.buckets().enumerate() {
			if !bucket.slots.is_empty() {
				let keep: Vec<(u32, PRef)> = bucket.slots.iter().filter(|(_, p)| p < &pref).cloned().collect();
				to_update.push((link_pref, keep));
				highest_used_bucket = bucket_number;
			}
		}

		for (pref, keep) in to_update.iter() {
			let links = Link::from_slots(&keep);
			let payload = Link::deserialize(links.as_slice()).to_payload();
			self.link_file.update(*pref, payload)?;
		}

		if let Some(link_pref) = self.link_prefs.get(highest_used_bucket) {
			if link_pref.is_valid() {
				self.link_file.truncate(link_pref.next_page().as_u64())?;
			}
		}
		// self.link_prefs.truncate(highest_used_bucket + 1);
		// self.table_file.truncate(TableFile::table_offset(highest_used_bucket).as_u64())?;

		self.batch()?;
		Ok(())
	}

	fn bucket_for_hash(&self, hash: u32) -> usize {
		let mut bucket = (hash & (!0u32 >> (32 - self.log_mod))) as usize; // hash % 2^(log_mod)
		if bucket < self.step {
			bucket = (hash & (!0u32 >> (32 - self.log_mod - 1))) as usize; // hash % 2^(log_mod + 1)
		}
		bucket
	}

	fn hash(&self, key: &[u8]) -> u32 {
		siphash24::Hash::hash_to_u64_with_keys(self.sip0, self.sip1, key) as u32
	}
}

#[derive(Clone, Default)]
pub struct Bucket {
	pub slots: Vec<(u32, PRef)>,
}

pub struct BucketIterator<'a> {
	file: &'a mut MemTable,
	n: usize,
}

impl<'a> Iterator for BucketIterator<'a> {
	type Item = (PRef, Bucket);

	fn next(&mut self) -> Option<<Self as Iterator>::Item> {
		self.file.resolve_bucket(self.n).unwrap();
		if let Some(pref) = self.file.link_prefs.get(self.n) {
			if let Some(bucket) = self.file.buckets.write().get(&self.n) {
				self.n += 1;
				return Some((*pref, bucket.clone()));
			}
		}
		None
	}
}

struct Dirty {
	bits: Vec<u64>,
	used: usize,
}

impl fmt::Debug for Dirty {
	fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
		for b in &self.bits {
			write!(f, "{:064b}", b)?;
		}
		Ok(())
	}
}

impl Dirty {
	pub fn new(n: usize) -> Dirty {
		Dirty {
			bits: vec![0u64; (n >> 6) + 1],
			used: n,
		}
	}

	pub fn set(&mut self, n: usize) {
		self.bits[n >> 6] |= 1 << (n & 0x3f);
	}

	pub fn unset(&mut self, n: usize) {
		self.bits[n >> 6] ^= 1 << (n & 0x3f);
	}

	pub fn get(&self, n: usize) -> bool {
		(self.bits[n >> 6] & (1 << (n & 0x3f))) != 0
	}

	pub fn is_dirty(&self) -> bool {
		self.bits.iter().any(|n| *n != 0)
	}

	pub fn append(&mut self) {
		self.used += 1;
		if self.used >= (self.bits.len() << 6) {
			self.bits.push(1);
		} else {
			let next = self.used;
			self.set(next);
		}
	}
}

struct DirtyIterator<'b> {
	bits: &'b Dirty,
	pos: usize,
}

impl<'b> DirtyIterator<'b> {
	pub fn new(bits: &'b Dirty) -> DirtyIterator<'b> {
		DirtyIterator { bits, pos: 0 }
	}
}

impl<'b> Iterator for DirtyIterator<'b> {
	type Item = bool;

	fn next(&mut self) -> Option<<Self as Iterator>::Item> {
		if self.pos < self.bits.used {
			let pos = self.pos;
			self.pos += 1;
			return Some(self.bits.get(pos));
		}
		return None;
	}
}

#[cfg(test)]
mod test {
	extern crate rand;

	use crate::transient::Transient;

	use self::rand::thread_rng;
	use self::rand::RngCore;
	use super::*;
	use std::collections::HashMap;

	#[test]
	fn test_dirty() {
		let mut dirty = Dirty::new(63);
		assert_eq!(
			format!("{:?}", dirty),
			"0000000000000000000000000000000000000000000000000000000000000000"
		);
		dirty.set(0);
		assert!(dirty.get(0));
		assert_eq!(
			format!("{:?}", dirty),
			"0000000000000000000000000000000000000000000000000000000000000001"
		);
		dirty.set(3);
		assert_eq!(
			format!("{:?}", dirty),
			"0000000000000000000000000000000000000000000000000000000000001001"
		);
		dirty.append();
		assert_eq!(format!("{:?}", dirty), "00000000000000000000000000000000000000000000000000000000000010010000000000000000000000000000000000000000000000000000000000000001");
		dirty.append();
		assert_eq!(format!("{:?}", dirty), "00000000000000000000000000000000000000000000000000000000000010010000000000000000000000000000000000000000000000000000000000000011");
		assert!(dirty.get(65));

		dirty.unset(3);
		assert_eq!(format!("{:?}", dirty), "00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000011");
		dirty.unset(64);
		assert_eq!(format!("{:?}", dirty), "00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000010");
	}

	#[test]
	fn test_many() {
		let mut db = Transient::new_db(1).unwrap();

		let mut rng = thread_rng();
		let mut key = [0x0u8; 32];
		let mut data = [0x0u8; 40];
		let mut check = HashMap::new();

		for _ in 0..10000 {
			rng.fill_bytes(&mut key);
			rng.fill_bytes(&mut data);
			let o = db.put_keyed(&key, &data).unwrap();
			check.insert(key, (o, data.to_vec()));
		}
		db.batch().unwrap();

		for (k, (o, data)) in &check {
			assert_eq!(db.get_keyed(&k[..]).unwrap().unwrap(), (*o, data.clone()));
		}

		for (k, (_, _)) in &check {
			db.forget(k).unwrap();
			assert!(db.get_keyed(&k[..]).unwrap().is_none());
		}

		db.shutdown().unwrap();
	}
}
