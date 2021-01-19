use crate::async_file::AsyncFile;
use crate::cached_file::CachedFile;
use crate::data_file::DataFile;
use crate::error::Error;
use crate::hammersbald_api::{Hammersbald, HammersbaldAPI};
use crate::log_file::LogFile;
use crate::page::{Page, PAGE_SIZE};
use crate::paged_file::PagedFile;
use crate::pref::PRef;
use crate::table_file::TableFile;

use parking_lot::Mutex;
use std::cmp::min;
use std::io;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;

/// create a transient db
pub fn transient() -> Result<Box<dyn HammersbaldAPI>, Error> {
	Transient::new_db(1)
}

/// in memory representation of a file
pub struct Transient {
	inner: Mutex<Inner>,
}

struct Inner {
	data: Vec<u8>,
	pos: usize,
}

impl Transient {
	/// create a new file
	fn new() -> Transient {
		Transient {
			inner: Mutex::new(Inner { data: Vec::new(), pos: 0 }),
		}
	}

	pub fn new_db(cached_data_pages: usize) -> Result<Box<dyn HammersbaldAPI>, Error> {
		let log = LogFile::new(Box::new(AsyncFile::new(Box::new(Transient::new()), "log")?));
		let table = TableFile::new(Box::new(AsyncFile::new(
			Box::new(CachedFile::new(Box::new(Transient::new()), cached_data_pages)?),
			"table",
		)?))?;
		let data = DataFile::new(Box::new(CachedFile::new(
			Box::new(AsyncFile::new(Box::new(Transient::new()), "data")?),
			cached_data_pages,
		)?))?;
		let link = DataFile::new(Box::new(CachedFile::new(
			Box::new(AsyncFile::new(Box::new(Transient::new()), "link")?),
			cached_data_pages,
		)?))?;
		Ok(Box::new(Hammersbald::new("", log, table, data, link)?))
	}
}

impl PagedFile for Transient {
	fn read_page(&self, pref: PRef) -> Result<Option<Page>, Error> {
		let mut inner = self.inner.lock();
		let len = inner.seek(SeekFrom::End(0))?;
		if pref.as_u64() < len {
			inner.seek(SeekFrom::Start(pref.as_u64()))?;
			let mut buffer = [0u8; PAGE_SIZE];
			inner.read(&mut buffer)?;
			return Ok(Some(Page::from_buf(buffer)));
		}
		Ok(None)
	}

	fn len(&self) -> Result<u64, Error> {
		let inner = self.inner.lock();
		Ok(inner.data.len() as u64)
	}

	fn truncate(&mut self, len: u64) -> Result<(), Error> {
		let mut inner = self.inner.lock();
		inner.data.truncate(len as usize);
		Ok(())
	}

	fn sync(&self) -> Result<(), Error> {
		Ok(())
	}

	fn shutdown(&mut self) -> Result<(), Error> {
		Ok(())
	}

	fn update_page(&mut self, page: Page) -> Result<u64, Error> {
		let mut inner = self.inner.lock();
		if page.pref().as_u64() <= inner.data.len() as u64 {
			inner.seek(SeekFrom::Start(page.pref().as_u64()))?;
		}
		inner.write(&page.into_buf())?;
		Ok(inner.data.len() as u64)
	}

	fn flush(&mut self) -> Result<(), Error> {
		Ok(())
	}
}

impl Read for Inner {
	fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
		let buflen = buf.len();
		if self.pos + buflen > self.data.len() {
			return Err(io::Error::from(io::ErrorKind::NotFound));
		}
		buf.copy_from_slice(&self.data.as_slice()[self.pos..self.pos + buflen]);
		self.pos += buflen;
		Ok(buflen)
	}
}

impl Write for Inner {
	fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
		let buflen = buf.len();

		let len = self.data.len();
		let pos = self.pos;
		let have = min(buflen, len - pos);
		self.data.as_mut_slice()[pos..pos + have].copy_from_slice(&buf[0..have]);
		if buflen > have {
			self.data.extend_from_slice(&buf[have..buflen]);
		}
		Ok(self.pos)
	}

	fn flush(&mut self) -> Result<(), io::Error> {
		Ok(())
	}
}

impl Seek for Inner {
	fn seek(&mut self, pos: SeekFrom) -> Result<u64, io::Error> {
		match pos {
			SeekFrom::Start(o) => {
				if o > self.data.len() as u64 {
					return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
				}
				self.pos = o as usize;
			}
			SeekFrom::Current(o) => {
				let newpos = o + self.pos as i64;
				if newpos < 0 || newpos > self.data.len() as i64 {
					return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
				}
				self.pos = newpos as usize;
			}
			SeekFrom::End(o) => {
				let newpos = o + self.data.len() as i64;
				if newpos < 0 || newpos > self.data.len() as i64 {
					return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
				}
				self.pos = newpos as usize;
			}
		}
		Ok(self.pos as u64)
	}
}
