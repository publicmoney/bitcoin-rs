use crate::error::Error;
use crate::page::Page;
use crate::paged_file::PagedFile;
use crate::pref::{PRef, PREF_SIZE};

pub struct LogFile {
	file: Box<dyn PagedFile>,
}

impl LogFile {
	pub fn new(rw: Box<dyn PagedFile>) -> LogFile {
		LogFile { file: rw }
	}

	pub fn init(&mut self, data_len: u64, table_len: u64, link_len: u64) -> Result<(), Error> {
		self.truncate(0)?;
		let mut first = Page::new();
		first.write_pref(0, PRef::from(data_len));
		first.write_pref(PREF_SIZE, PRef::from(table_len));
		first.write_pref(PREF_SIZE * 2, PRef::from(link_len));

		self.file.update_page(first)?;
		Ok(())
	}

	pub fn recover(&self) -> Result<(u64, u64, u64), Error> {
		if let Some(page) = self.read_page(PRef::from(0))? {
			let data_len = page.read_pref(0).as_u64();
			let table_len = page.read_pref(PREF_SIZE).as_u64();
			let link_len = page.read_pref(PREF_SIZE * 2).as_u64();
			return Ok((data_len, table_len, link_len));
		}
		Ok((0, 0, 0))
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
		self.file.shutdown()
	}

	fn update_page(&mut self, _: Page) -> Result<u64, Error> {
		unreachable!()
	}

	fn flush(&mut self) -> Result<(), Error> {
		Ok(self.file.flush()?)
	}
}
