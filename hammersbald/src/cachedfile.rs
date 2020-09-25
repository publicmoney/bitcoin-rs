use crate::error::Error;
use crate::page::{Page, PAGE_SIZE};
use crate::pagedfile::PagedFile;
use crate::pref::PRef;

use lru_cache::LruCache;

use std::cmp::max;
use std::sync::{Arc, Mutex};

pub struct CachedFile {
	file: Box<dyn PagedFile>,
	cache: Mutex<Cache>,
}

impl CachedFile {
	/// create a read cached file with a page cache of given size
	pub fn new(file: Box<dyn PagedFile>, cache_size_mb: usize) -> Result<CachedFile, Error> {
		let pages = cache_size_mb * 1_000_000 / PAGE_SIZE;
		let len = file.len()?;
		Ok(CachedFile {
			file,
			cache: Mutex::new(Cache::new(len, pages)),
		})
	}
}

impl PagedFile for CachedFile {
	fn read_page(&self, pref: PRef) -> Result<Option<Page>, Error> {
		let mut cache = self.cache.lock().unwrap();
		if let Some(page) = cache.get(pref) {
			return Ok(Some(page));
		}
		if let Some(page) = self.file.read_page(pref)? {
			cache.cache(pref, Arc::new(page.clone()));
			return Ok(Some(page));
		}
		Ok(None)
	}

	fn len(&self) -> Result<u64, Error> {
		self.file.len()
	}

	fn truncate(&mut self, new_len: u64) -> Result<(), Error> {
		self.cache.lock().unwrap().reset_len(new_len);
		Ok(())
	}

	fn sync(&self) -> Result<(), Error> {
		self.file.sync()
	}

	fn shutdown(&mut self) -> Result<(), Error> {
		self.file.shutdown()
	}

	fn update_page(&mut self, page: Page) -> Result<u64, Error> {
		let mut cache = self.cache.lock().unwrap();
		cache.update(page.clone());
		self.file.update_page(page)
	}

	fn flush(&mut self) -> Result<(), Error> {
		self.cache.lock().unwrap().clear();
		self.file.flush()
	}
}

pub struct Cache {
	reads: LruCache<PRef, Arc<Page>>,
	len: u64,
}

impl Cache {
	pub fn new(len: u64, size: usize) -> Cache {
		Cache {
			reads: LruCache::new(size),
			len,
		}
	}

	pub fn cache(&mut self, pref: PRef, page: Arc<Page>) {
		self.reads.insert(pref, page);
	}

	pub fn clear(&mut self) {
		self.reads.clear();
	}

	pub fn update(&mut self, page: Page) -> u64 {
		let pref = page.pref();
		let page = Arc::new(page);
		self.cache(pref, page);
		self.len = max(self.len, pref.as_u64() + PAGE_SIZE as u64);
		self.len
	}

	pub fn get(&mut self, pref: PRef) -> Option<Page> {
		use std::ops::Deref;
		if let Some(content) = self.reads.get_mut(&pref) {
			return Some(content.clone().deref().clone());
		}
		None
	}

	pub fn reset_len(&mut self, len: u64) {
		self.len = len;
		let to_delete: Vec<_> = self
			.reads
			.iter()
			.filter_map(|(o, _)| {
				let l = o.as_u64();
				if l >= len {
					Some(l)
				} else {
					None
				}
			})
			.collect();
		for o in to_delete {
			self.reads.remove(&PRef::from(o));
		}
	}
}
