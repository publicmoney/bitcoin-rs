use crate::error::Error;
use crate::page::{Page, PAGE_PAYLOAD_SIZE, PAGE_SIZE};
use crate::pref::{PRef, PREF_SIZE};

use std::cmp::min;
use std::io::{self, ErrorKind};

/// a paged file
pub trait PagedFile: Send + Sync {
	/// read a page at pref
	fn read_page(&self, pref: PRef) -> Result<Option<Page>, Error>;
	/// length of the storage
	fn len(&self) -> Result<u64, Error>;
	/// truncate storage
	fn truncate(&mut self, new_len: u64) -> Result<(), Error>;
	/// tell OS to flush buffers to disk
	fn sync(&self) -> Result<(), Error>;
	/// shutdown async write
	fn shutdown(&mut self) -> Result<(), Error>;
	/// write a page at its position
	fn update_page(&mut self, page: Page) -> Result<u64, Error>;
	/// flush buffered writes
	fn flush(&mut self) -> Result<(), Error>;
}

/// Reads and writes buffers to pages.
pub struct PagedFileAppender {
	file: Box<dyn PagedFile>,
	pos: PRef,
	current_page: Option<Page>,
}

impl PagedFileAppender {
	pub fn new(file: Box<dyn PagedFile>, pos: PRef) -> PagedFileAppender {
		PagedFileAppender {
			file,
			pos,
			current_page: None,
		}
	}

	pub fn position(&self) -> PRef {
		self.pos
	}

	pub fn append(&mut self, buf: &[u8]) -> Result<PRef, Error> {
		self.pos = self.update(self.pos, buf)?;
		Ok(self.pos)
	}

	pub fn update(&mut self, pos: PRef, buf: &[u8]) -> Result<PRef, Error> {
		let mut new_pos = pos;
		let mut wrote = 0;

		while wrote < buf.len() {
			let page_pref = new_pos.this_page();

			let mut page = if self.current_page.as_ref().filter(|page| page.pref() == page_pref).is_some() {
				self.current_page.take().unwrap()
			} else {
				self.file
					.read_page(page_pref)?
					.unwrap_or_else(|| Page::new_page_with_position(page_pref))
			};

			let in_page_pos = new_pos.in_page_pos();
			let space = min(PAGE_PAYLOAD_SIZE - in_page_pos, buf.len() - wrote);
			page.write(in_page_pos, &buf[wrote..wrote + space]);

			wrote += space;
			new_pos += space as u64;

			if new_pos.in_page_pos() == PAGE_PAYLOAD_SIZE {
				new_pos += PREF_SIZE as u64;
			}

			if self.current_page.is_none() && new_pos > self.pos && new_pos.in_page_pos() != 0 {
				self.current_page = Some(page);
			} else {
				self.update_page(page)?;
			}
		}
		Ok(new_pos)
	}

	pub fn read(&self, pos: PRef, buf: &mut [u8]) -> Result<PRef, Error> {
		let mut pos = pos;
		let mut read = 0;

		while read < buf.len() {
			if let Some(page) = self
				.current_page
				.as_ref()
				.filter(|page| pos.this_page() == page.pref())
				.or(self.file.read_page(pos.this_page())?.as_ref())
			{
				let have = min(PAGE_PAYLOAD_SIZE - pos.in_page_pos(), buf.len() - read);
				page.read(pos.in_page_pos(), &mut buf[read..read + have]);
				read += have;
				pos += have as u64;
				if pos.in_page_pos() == PAGE_PAYLOAD_SIZE {
					pos += PREF_SIZE as u64;
				}
			} else {
				return Err(Error::IO(io::Error::from(ErrorKind::UnexpectedEof)));
			}
		}
		Ok(pos)
	}
}

impl PagedFile for PagedFileAppender {
	fn read_page(&self, pref: PRef) -> Result<Option<Page>, Error> {
		if let Some(ref page) = self.current_page {
			if pref.this_page() == page.pref() {
				return Ok(Some(page.clone()));
			}
		}
		self.file.read_page(pref)
	}

	fn len(&self) -> Result<u64, Error> {
		Ok(self.pos.as_u64())
	}

	fn truncate(&mut self, new_len: u64) -> Result<(), Error> {
		self.pos = PRef::from(new_len);
		self.file.truncate(new_len)
	}

	fn sync(&self) -> Result<(), Error> {
		self.file.sync()
	}

	fn shutdown(&mut self) -> Result<(), Error> {
		self.file.shutdown()
	}

	fn update_page(&mut self, page: Page) -> Result<u64, Error> {
		self.file.update_page(page)
	}

	fn flush(&mut self) -> Result<(), Error> {
		if let Some(page) = &self.current_page {
			if self.pos.in_page_pos() > 0 {
				self.file.update_page(page.clone())?;
			}
		}
		Ok(self.file.flush()?)
	}
}

/// iterate through pages of a paged file
pub struct PagedFileIterator<'file> {
	// the current page of the iterator
	pagenumber: u64,
	// the iterated file
	file: &'file dyn PagedFile,
}

impl<'file> Iterator for PagedFileIterator<'file> {
	type Item = Page;

	fn next(&mut self) -> Option<Self::Item> {
		if self.pagenumber <= (1 << 35) / PAGE_SIZE as u64 {
			let pref = PRef::from((self.pagenumber) * PAGE_SIZE as u64);
			if let Ok(Some(page)) = self.file.read_page(pref) {
				self.pagenumber += 1;
				return Some(page);
			}
		}
		None
	}
}

#[cfg(test)]
mod tests {
	use crate::page::PAGE_SIZE;
	use crate::paged_file::{PagedFile, PagedFileAppender};
	use crate::pref::PRef;
	use crate::pref::PREF_SIZE;
	use crate::rolled_file::RolledFile;
	use std::fs;

	#[test]
	fn test_append_update() {
		fs::remove_dir_all("testdb/paged-append").unwrap_or_default();

		let rolled_file = RolledFile::new("testdb/paged-append", "test", "bc", PAGE_SIZE as u64).unwrap();
		let mut appender = PagedFileAppender::new(Box::new(rolled_file), PRef::from(0));

		let value = [1, 2, 3];
		appender.append(&value).unwrap();

		let result = appender.read_page(PRef::from(0)).unwrap().unwrap();
		let mut res = [0u8; 3];
		result.read(0, &mut res);

		assert_eq!(3, appender.len().unwrap());
		assert_eq!(value, res);

		appender.update(PRef::from(2), &[5]).unwrap();
		let result = appender.read_page(PRef::from(0)).unwrap().unwrap();
		let mut res = [0u8; 3];
		result.read(0, &mut res);
		assert_eq!([1, 2, 5], res);
	}

	#[test]
	fn test_big() {
		fs::remove_dir_all("testdb/paged-big").unwrap_or_default();

		let rolled_file = RolledFile::new("testdb/paged-big", "test", "bc", PAGE_SIZE as u64).unwrap();
		let mut appender = PagedFileAppender::new(Box::new(rolled_file), PRef::from(0));

		let value = [1u8; 5000];
		appender.append(&value).unwrap();

		let mut res = [0u8; 5000];
		appender.read(PRef::from(0), &mut res).unwrap();

		assert_eq!(5000 + PREF_SIZE as u64, appender.len().unwrap());
		for i in 0..500 {
			assert_eq!([1u8; 10], res[i * 10..i * 10 + 10]);
		}
	}
}
