use crate::error::Error;
use crate::page::Page;
use crate::paged_file::PagedFile;
use crate::pref::PRef;
use crate::single_file::SingleFile;

use std::cmp::max;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::path::Path;

pub struct RolledFile {
	name: String,
	extension: String,
	files: HashMap<u16, SingleFile>,
	len: u64,
	file_size: u64,
}

impl RolledFile {
	pub fn new(name: &str, extension: &str, file_size: u64) -> Result<RolledFile, Error> {
		let mut rolled = RolledFile {
			name: name.to_string(),
			extension: extension.to_string(),
			files: HashMap::new(),
			len: 0,
			file_size,
		};
		rolled.open()?;
		Ok(rolled)
	}

	fn open(&mut self) -> Result<(), Error> {
		// interesting file names are:
		// name.index.extension
		// where index is a number
		if let Some(basename) = Path::new(self.name.as_str()).file_name() {
			let mut highest_index = 0;
			if let Some(mut dir) = Path::new(&self.name).parent() {
				if dir.to_string_lossy().to_string().is_empty() {
					dir = Path::new(".");
				}
				for entry in fs::read_dir(dir)? {
					let path = entry?.path();
					if path.is_file() {
						if let Some(name_index) = path.file_stem() {
							// name.index
							let ni = Path::new(name_index.clone());
							if let Some(name) = ni.file_stem() {
								// compare name
								if name == basename {
									// compare extension
									if let Some(extension) = path.extension() {
										if extension.to_string_lossy().to_string() == self.extension {
											// parse index
											if let Some(index) = ni.extension() {
												if let Ok(number) = index.to_string_lossy().parse::<u16>() {
													let filename = path.clone().to_string_lossy().to_string();
													let file = Self::open_file(filename)?;
													self.files.insert(
														number,
														SingleFile::new(file, number as u64 * self.file_size, self.file_size)?,
													);
													if let Some(file) = self.files.get(&number) {
														if file.len().unwrap() > 0 {
															highest_index = max(highest_index, number);
														}
													}
												}
											}
										}
									}
								}
							}
						}
					}
				}
			}
			if let Some(file) = self.files.get(&highest_index) {
				self.len = highest_index as u64 * self.file_size + file.len()?;
			}
		} else {
			return Err(Error::Corrupted("invalid db name".to_string()));
		}
		Ok(())
	}

	fn open_file(path: String) -> Result<File, Error> {
		let mut open_mode = OpenOptions::new();
		open_mode.read(true).write(true).create(true);
		Ok(open_mode.open(path)?)
	}
}

impl PagedFile for RolledFile {
	fn read_page(&self, pref: PRef) -> Result<Option<Page>, Error> {
		if pref.as_u64() <= self.len {
			let file_index = (pref.as_u64() / self.file_size) as u16;
			if let Some(file) = self.files.get(&file_index) {
				return file.read_page(pref);
			}
		}
		Ok(None)
	}

	// The total length across several files (a multiple of PAGE_SIZE)
	fn len(&self) -> Result<u64, Error> {
		Ok(self.len)
	}

	fn truncate(&mut self, new_len: u64) -> Result<(), Error> {
		let file_index = (new_len / self.file_size) as u16;
		for (c, file) in &mut self.files {
			if *c > file_index {
				file.truncate(0)?;
			}
		}
		if let Some(last) = self.files.get_mut(&file_index) {
			last.truncate(new_len % self.file_size)?;
		}
		self.len = new_len;
		Ok(())
	}

	fn sync(&self) -> Result<(), Error> {
		for file in self.files.values() {
			file.sync()?;
		}
		Ok(())
	}

	fn shutdown(&mut self) -> Result<(), Error> {
		Ok(())
	}

	fn update_page(&mut self, page: Page) -> Result<u64, Error> {
		let n_offset = page.pref().as_u64();
		let file_index = (n_offset / self.file_size) as u16;

		if !self.files.contains_key(&file_index) {
			let file = Self::open_file((((self.name.clone() + ".") + file_index.to_string().as_str()) + ".") + self.extension.as_str())?;
			self.files.insert(
				file_index,
				SingleFile::new(file, (n_offset / self.file_size) * self.file_size, self.file_size)?,
			);
		}

		if let Some(file) = self.files.get_mut(&file_index) {
			self.len = max(self.len, file.update_page(page)? + file_index as u64 * self.file_size);
			Ok(self.len)
		} else {
			return Err(Error::Corrupted(format!("missing file index in write {}", file_index)));
		}
	}

	fn flush(&mut self) -> Result<(), Error> {
		for file in &mut self.files.values_mut() {
			file.flush()?;
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::RolledFile;
	use crate::page::{Page, PAGE_SIZE};
	use crate::paged_file::PagedFile;
	use crate::pref::PRef;
	use std::fs;

	#[test]
	#[allow(unused_must_use)]
	fn test_rolled_file() {
		fs::remove_file("rolled-test.0.bc");
		fs::remove_file("rolled-test.1.bc");

		let mut rolled_file = RolledFile::new("rolled-test", "bc", PAGE_SIZE as u64).unwrap();

		let page_one_pref = PRef::from(0);
		let mut page_one = Page::new_page_with_position(page_one_pref);
		page_one.write_u64(0, 1);
		rolled_file.update_page(page_one.clone()).unwrap();

		let page_two_pref = page_one_pref.next_page();
		let mut page_two = Page::new_page_with_position(page_two_pref);
		page_two.write_u64(0, 2);
		rolled_file.update_page(page_two.clone()).unwrap();

		rolled_file.update_page(page_one.clone()).unwrap();

		rolled_file.sync().unwrap();
		rolled_file.flush().unwrap();

		assert_eq!(
			PAGE_SIZE as u64,
			fs::File::open("rolled-test.0.bc").unwrap().metadata().unwrap().len()
		);
		assert_eq!(
			PAGE_SIZE as u64,
			fs::File::open("rolled-test.1.bc").unwrap().metadata().unwrap().len()
		);
		assert!(fs::File::open("rolled-test.2.bc").is_err());
	}
}
