use crate::error::Error;
use crate::page::Page;
use crate::pagedfile::{PagedFile, PagedFileIterator};
use crate::pref::{PRef, PREF_SIZE};

use std::collections::HashSet;

pub struct LogFile {
	file: Box<dyn PagedFile>,
	logged: HashSet<PRef>,
	source_len: u64,
}

impl LogFile {
	pub fn new(rw: Box<dyn PagedFile>) -> LogFile {
		LogFile {
			file: rw,
			logged: HashSet::new(),
			source_len: 0,
		}
	}

	pub fn init(&mut self, data_len: u64, table_len: u64, link_len: u64) -> Result<(), Error> {
		self.truncate(0)?;
		let mut first = Page::new();
		first.write_pref(0, PRef::from(data_len));
		first.write_pref(PREF_SIZE, PRef::from(table_len));
		first.write_pref(PREF_SIZE * 2, PRef::from(link_len));

		self.file.update_page(first)?;
		self.flush()?;
		Ok(())
	}

	pub fn page_iter(&self) -> PagedFileIterator {
		PagedFileIterator::new(self, PRef::from(0))
	}

	pub fn log_page(&mut self, pref: PRef, source: &dyn PagedFile) -> Result<(), Error> {
		if pref.as_u64() < self.source_len && self.logged.insert(pref) {
			if let Some(page) = source.read_page(pref)? {
				self.file.update_page(page)?;
			}
		}
		Ok(())
	}

	pub fn reset(&mut self, len: u64) {
		self.source_len = len;
		self.logged.clear();
	}
}

impl PagedFile for LogFile {
	fn read_page(&self, pref: PRef) -> Result<Option<Page>, Error> {
		self.file.read_page(pref)
	}

	fn len(&self) -> Result<u64, Error> {
		self.file.len()
	}

	fn truncate(&mut self, len: u64) -> Result<(), Error> {
		self.file.truncate(len)
	}

	fn sync(&self) -> Result<(), Error> {
		self.file.sync()
	}

	fn shutdown(&mut self) -> Result<(), Error> {
		Ok(())
	}

	fn update_page(&mut self, _: Page) -> Result<u64, Error> {
		unreachable!()
	}

	fn flush(&mut self) -> Result<(), Error> {
		Ok(self.file.flush()?)
	}
}
