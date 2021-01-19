use crate::error::Error;
use crate::page::{Page, PAGE_PAYLOAD_SIZE, PAGE_SIZE};
use crate::paged_file::PagedFile;
use crate::pref::PRef;

use parking_lot::Mutex;
use std::cmp::max;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

pub struct SingleFile {
	path: String,
	file: Mutex<File>,
	base: u64,
	len: u64,
	file_size: u64,
}

impl SingleFile {
	pub fn new(path: String, base: u64, file_size: u64) -> Result<SingleFile, Error> {
		let mut file = SingleFile::open_file(path.clone())?;
		let len = file.seek(SeekFrom::End(0))?;
		Ok(SingleFile {
			path,
			file: Mutex::new(file),
			base,
			len,
			file_size,
		})
	}

	pub fn delete(&self) {
		std::fs::remove_file(&self.path).unwrap()
	}

	fn open_file(path: String) -> Result<File, Error> {
		let mut open_mode = OpenOptions::new();
		open_mode.read(true).write(true).create(true);
		Ok(open_mode.open(path)?)
	}
}

impl PagedFile for SingleFile {
	fn read_page(&self, pref: PRef) -> Result<Option<Page>, Error> {
		let pos = pref.as_u64();
		if pos < self.base || pos >= self.base + self.file_size {
			return Err(Error::Corrupted(format!("read from wrong file {}", self.path).to_string()));
		}
		let pos = pos - self.base;
		if pos < self.len {
			let mut buffer = [0u8; PAGE_SIZE];
			let mut file = self.file.lock();
			file.seek(SeekFrom::Start(pos))?;
			file.read_exact(&mut buffer)?;
			return Ok(Some(Page::from_buf(buffer)));
		}
		Ok(None)
	}

	fn len(&self) -> Result<u64, Error> {
		Ok(self.len)
	}

	fn truncate(&mut self, new_len: u64) -> Result<(), Error> {
		if new_len < self.len {
			let pref = PRef::from(new_len);
			if let Some(mut page) = self.read_page(pref.this_page() + self.base)? {
				self.file.lock().set_len(pref.this_page().next_page().as_u64())?;
				let buf = [0u8; PAGE_PAYLOAD_SIZE];
				page.write(pref.in_page_pos(), &buf[..PAGE_PAYLOAD_SIZE - pref.in_page_pos()]);
				self.update_page(page)?;
				self.len = new_len;
			};
		}
		Ok(())
	}

	fn sync(&self) -> Result<(), Error> {
		Ok(self.file.lock().sync_data()?)
	}

	fn shutdown(&mut self) -> Result<(), Error> {
		Ok(())
	}

	fn update_page(&mut self, page: Page) -> Result<u64, Error> {
		let page_pos = page.pref().as_u64();
		if page_pos < self.base || page_pos >= self.base + self.file_size {
			return Err(Error::Corrupted("write to wrong file".to_string()));
		}
		let pos = page_pos - self.base;

		let mut file = self.file.lock();
		file.seek(SeekFrom::Start(pos))?;
		file.write_all(&page.into_buf())?;
		self.len = max(self.len, pos + PAGE_SIZE as u64);
		Ok(self.len)
	}

	fn flush(&mut self) -> Result<(), Error> {
		Ok(self.file.lock().flush()?)
	}
}

#[cfg(test)]
mod tests {
	use crate::page::{Page, PAGE_SIZE};
	use crate::paged_file::PagedFile;
	use crate::pref::PRef;
	use crate::single_file::SingleFile;
	use std::fs;

	#[test]
	fn test_single_file() {
		fs::remove_dir_all("testdb/single").unwrap_or_default();
		fs::create_dir_all("testdb/single").unwrap_or_default();

		let mut single_file = SingleFile::new("testdb/single/test.bc".to_string(), 0, 100000).unwrap();

		let page_one_pref = PRef::from(0);
		let mut page_one = Page::new_page_with_position(page_one_pref);
		page_one.write_u64(0, 1);
		single_file.update_page(page_one.clone()).unwrap();

		let page_two_pref = page_one_pref.next_page();
		let mut page_two = Page::new_page_with_position(page_two_pref);
		page_two.write_u64(0, 2);
		single_file.update_page(page_two.clone()).unwrap();

		single_file.sync().unwrap();
		single_file.flush().unwrap();
		single_file.truncate(PAGE_SIZE as u64 + 100).unwrap();

		assert_eq!(PAGE_SIZE as u64 + 100, single_file.len);

		page_one.write_u64(0, 3);
		single_file.update_page(page_one.clone()).unwrap();
		single_file.update_page(page_two.clone()).unwrap();

		assert_eq!(PAGE_SIZE as u64 * 2, single_file.len);

		let page_result = single_file.read_page(page_one_pref).unwrap().unwrap();
		assert_eq!(3, page_result.read_u64(0))
	}
}
