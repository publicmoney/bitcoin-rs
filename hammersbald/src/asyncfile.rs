//
// Copyright 2018-2019 Tamas Blummer
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
//!
//! # Asynchronous file
//! an append only file written in background
//!
use crate::page::Page;
use crate::pagedfile::PagedFile;
use crate::{Error, PRef};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

pub struct AsyncFile {
	inner: Arc<AsyncFileInner>,
}

#[derive(Clone)]
enum Operation {
	Append(Page),
	Update(Page),
}

struct AsyncFileInner {
	file: Mutex<Box<dyn PagedFile + Send + Sync>>,
	work: Condvar,
	flushed: Condvar,
	run: AtomicBool,
	queue: Mutex<Vec<Operation>>,
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
		let mut queue = inner.queue.lock().expect("page queue lock poisoned");
		while inner.run.load(Ordering::Acquire) {
			while queue.is_empty() {
				queue = inner.work.wait(queue).expect("page queue lock poisoned");
			}
			let mut file = inner.file.lock().expect("file lock poisoned");
			for operation in queue.iter() {
				match operation {
					Operation::Append(page) => file.append_page(page.clone()).expect("can not write in background"),
					Operation::Update(page) => {
						file.update_page(page.clone()).expect("can not write in background");
					}
				}
			}
			queue.clear();
			inner.flushed.notify_all();
		}
	}

	fn read_in_queue(&self, pref: PRef) -> Result<Option<Page>, Error> {
		let queue = self.inner.queue.lock().expect("page queue lock poisoned");
		if queue.len() > 0 {
			let mut rev_q = queue.clone();
			rev_q.reverse();
			for operation in rev_q.iter() {
				let page = match operation {
					Operation::Update(p) => p,
					Operation::Append(p) => p,
				};
				if page.pref() == pref {
					return Ok(Some(page.clone()));
				}
			}
		}
		Ok(None)
	}
}

impl PagedFile for AsyncFile {
	fn read_page(&self, pref: PRef) -> Result<Option<Page>, Error> {
		if let Some(page) = self.read_in_queue(pref)? {
			return Ok(Some(page));
		}
		let file = self.inner.file.lock().expect("file lock poisoned");
		file.read_page(pref)
	}

	fn len(&self) -> Result<u64, Error> {
		self.inner.file.lock().unwrap().len()
	}

	fn truncate(&mut self, new_len: u64) -> Result<(), Error> {
		self.inner.file.lock().unwrap().truncate(new_len)
	}

	fn sync(&self) -> Result<(), Error> {
		self.inner.file.lock().unwrap().sync()
	}

	fn shutdown(&mut self) -> Result<(), Error> {
		self.flush()?;
		self.inner.run.store(false, Ordering::Release);
		Ok(())
	}

	fn append_page(&mut self, page: Page) -> Result<(), Error> {
		let mut queue = self.inner.queue.lock().unwrap();
		queue.push(Operation::Append(page));
		self.inner.work.notify_one();
		Ok(())
	}

	fn update_page(&mut self, page: Page) -> Result<u64, Error> {
		let mut queue = self.inner.queue.lock().unwrap();
		queue.push(Operation::Update(page));
		self.inner.work.notify_one();
		Ok(0)
	}

	fn flush(&mut self) -> Result<(), Error> {
		let mut queue = self.inner.queue.lock().unwrap();
		self.inner.work.notify_one();
		while !queue.is_empty() {
			queue = self.inner.flushed.wait(queue).unwrap();
		}
		let mut file = self.inner.file.lock().unwrap();
		file.flush()
	}
}
