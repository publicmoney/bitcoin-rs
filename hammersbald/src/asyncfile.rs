use crate::page::Page;
use crate::pagedfile::PagedFile;
use crate::pref::PRef;
use crate::Error;
use parking_lot::{Condvar, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

pub struct AsyncFile {
	inner: Arc<AsyncFileInner>,
}

struct AsyncFileInner {
	file: Mutex<Box<dyn PagedFile + Send + Sync>>,
	work: Condvar,
	flushed: Condvar,
	run: AtomicBool,
	queue: Mutex<Vec<Page>>,
}

impl AsyncFileInner {
	pub fn new(file: Box<dyn PagedFile + Send + Sync>) -> Result<AsyncFileInner, Error> {
		Ok(AsyncFileInner {
			file: Mutex::new(file),
			flushed: Condvar::new(),
			work: Condvar::new(),
			run: AtomicBool::new(true),
			queue: Mutex::new(Vec::new()),
		})
	}
}

impl AsyncFile {
	pub fn new(file: Box<dyn PagedFile + Send + Sync>) -> Result<AsyncFile, Error> {
		let inner = Arc::new(AsyncFileInner::new(file)?);
		let inner2 = inner.clone();
		thread::Builder::new()
			.name("hammersbald".to_string())
			.spawn(move || AsyncFile::background(inner2))
			.expect("hammersbald can not start thread for async file IO");
		Ok(AsyncFile { inner })
	}

	fn background(inner: Arc<AsyncFileInner>) {
		let mut queue = inner.queue.lock();
		while inner.run.load(Ordering::Acquire) {
			while queue.is_empty() {
				inner.work.wait(&mut queue);
			}
			let mut file = inner.file.lock();

			for page in queue.iter() {
				file.update_page(page.clone()).expect("error in async file writer");
			}
			queue.clear();
			inner.flushed.notify_all();
		}
	}

	fn read_in_queue(&self, pref: PRef) -> Result<Option<Page>, Error> {
		let queue = self.inner.queue.lock();
		let mut result = None;
		// Get the latest update to the page.
		for page in queue.iter() {
			if page.pref() == pref.this_page() {
				result = Some(page);
			}
		}
		Ok(result.cloned())
	}
}

impl PagedFile for AsyncFile {
	fn read_page(&self, pref: PRef) -> Result<Option<Page>, Error> {
		if let Some(page) = self.read_in_queue(pref)? {
			return Ok(Some(page));
		}
		let file = self.inner.file.lock();
		file.read_page(pref)
	}

	fn len(&self) -> Result<u64, Error> {
		self.inner.file.lock().len()
	}

	fn truncate(&mut self, new_len: u64) -> Result<(), Error> {
		self.inner.file.lock().truncate(new_len)
	}

	fn sync(&self) -> Result<(), Error> {
		self.inner.file.lock().sync()
	}

	fn shutdown(&mut self) -> Result<(), Error> {
		self.flush()?;
		self.inner.run.store(false, Ordering::Release);
		Ok(())
	}

	fn update_page(&mut self, page: Page) -> Result<u64, Error> {
		let mut queue = self.inner.queue.lock();
		queue.push(page);
		self.inner.work.notify_one();
		Ok(0)
	}

	fn flush(&mut self) -> Result<(), Error> {
		let mut queue = self.inner.queue.lock();
		self.inner.work.notify_one();
		while !queue.is_empty() {
			self.inner.flushed.wait(&mut queue);
		}
		self.inner.file.lock().flush()
	}
}
