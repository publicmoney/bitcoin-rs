use crate::error::Error;
use crate::page::{Page, PAGE_SIZE};
use crate::paged_file::PagedFile;
use crate::pref::PRef;
use lru::LruCache;
use parking_lot::Mutex;

pub struct CachedFile {
	file: Box<dyn PagedFile>,
	cache: Mutex<Cache>,
}

impl CachedFile {
	/// create a read cached file with a page cache of given size
	pub fn new(file: Box<dyn PagedFile>, cache_size_mb: usize) -> Result<CachedFile, Error> {
		let pages = cache_size_mb * 1_000_000 / PAGE_SIZE;
		Ok(CachedFile {
			file,
			cache: Mutex::new(Cache::new(pages)),
		})
	}
}

impl PagedFile for CachedFile {
	fn read_page(&self, pref: PRef) -> Result<Option<Page>, Error> {
		let mut cache = self.cache.lock();
		if let Some(page) = cache.get(pref) {
			return Ok(Some(page));
		}
		if let Some(page) = self.file.read_page(pref)? {
			cache.cache(pref, page.clone());
			return Ok(Some(page));
		}
		Ok(None)
	}

	fn len(&self) -> Result<u64, Error> {
		self.file.len()
	}

	fn truncate(&mut self, new_len: u64) -> Result<(), Error> {
		self.cache.lock().reset_len(new_len);
		self.file.truncate(new_len)?;
		Ok(())
	}

	fn sync(&self) -> Result<(), Error> {
		self.file.sync()
	}

	fn shutdown(&mut self) -> Result<(), Error> {
		self.file.shutdown()
	}

	fn update_page(&mut self, page: Page) -> Result<u64, Error> {
		let mut cache = self.cache.lock();
		cache.update(page.clone());
		self.file.update_page(page)
	}

	fn flush(&mut self) -> Result<(), Error> {
		self.cache.lock().clear();
		self.file.flush()
	}
}

pub struct Cache {
	reads: LruCache<PRef, Page>,
}

impl Cache {
	pub fn new(size: usize) -> Cache {
		Cache {
			reads: LruCache::new(size),
		}
	}

	pub fn cache(&mut self, pref: PRef, page: Page) {
		self.reads.put(pref, page);
	}

	pub fn clear(&mut self) {
		self.reads.clear();
	}

	pub fn update(&mut self, page: Page) -> u64 {
		self.cache(page.pref(), page);
		self.reads.len() as u64
	}

	pub fn get(&mut self, pref: PRef) -> Option<Page> {
		if let Some(content) = self.reads.get(&pref) {
			return Some(content.clone());
		}
		None
	}

	pub fn reset_len(&mut self, len: u64) {
		let to_delete: Vec<u64> = self
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
			self.reads.pop(&PRef::from(o));
		}
	}
}
