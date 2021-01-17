use crate::error::Error;
use crate::page::Page;
use crate::paged_file::PagedFile;
use crate::pref::PRef;
use crate::single_file::SingleFile;

use std::cmp::max;
use std::collections::HashMap;
use std::path::Path;

// File names have the format name.index.extension where index is a number.
pub struct RolledFile {
	path: String,
	basename: String,
	extension: String,
	files: HashMap<u16, SingleFile>,
	len: u64,
	file_size: u64,
}

impl RolledFile {
	pub fn new(path: &str, name: &str, extension: &str, file_size: u64) -> Result<RolledFile, Error> {
		std::fs::create_dir_all(path)?;

		let mut rolled = RolledFile {
			path: path.to_string(),
			basename: name.to_string(),
			extension: extension.to_string(),
			files: HashMap::new(),
			len: 0,
			file_size,
		};
		rolled.open()?;
		Ok(rolled)
	}

	fn open(&mut self) -> Result<(), Error> {
		let mut highest_index = 0;

		for entry in std::fs::read_dir(&self.path)? {
			let path = entry?.path();
			if path.is_file() {
				if let Some(name_index) = path.file_stem() {
					// name.index
					let ni = Path::new(name_index.clone());
					if let Some(name) = ni.file_stem() {
						// compare name
						if name == self.basename.as_str() {
							// compare extension
							if let Some(extension) = path.extension() {
								if extension.to_string_lossy().to_string() == self.extension {
									// parse index
									if let Some(index) = ni.extension() {
										if let Ok(number) = index.to_string_lossy().parse::<u16>() {
											let filename = path.clone().to_string_lossy().to_string();
											self.files
												.insert(number, SingleFile::new(filename, number as u64 * self.file_size, self.file_size)?);
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
		if let Some(file) = self.files.get_mut(&highest_index) {
			self.len = highest_index as u64 * self.file_size + file.len()?;
		}
		Ok(())
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

		let to_delete: Vec<u16> = self.files.iter().filter(|(i, _)| **i > file_index).map(|file| *file.0).collect();

		for number in to_delete {
			self.files.remove(&number).unwrap().delete();
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
			let path = (((self.path.clone() + "/") + (self.basename.clone() + ".").as_str() + file_index.to_string().as_str()) + ".")
				+ self.extension.as_str();
			self.files.insert(
				file_index,
				SingleFile::new(path, (n_offset / self.file_size) * self.file_size, self.file_size)?,
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
	fn test_rolled_file() {
		fs::remove_dir_all("testdb/rolled").unwrap_or_default();

		let page_one_pref = PRef::from(0);
		{
			let mut rolled_file = RolledFile::new("testdb/rolled", "test", "bc", PAGE_SIZE as u64).unwrap();

			let mut page_one = Page::new_page_with_position(page_one_pref);
			page_one.write_u64(0, 1);
			rolled_file.update_page(page_one.clone()).unwrap();

			let mut page_two = Page::new_page_with_position(page_one_pref.next_page());
			page_two.write_u64(0, 2);
			rolled_file.update_page(page_two.clone()).unwrap();

			rolled_file.update_page(page_one.clone()).unwrap();

			rolled_file.sync().unwrap();
			rolled_file.flush().unwrap();
		}

		assert_eq!(
			PAGE_SIZE as u64,
			fs::File::open("testdb/rolled/test.0.bc").unwrap().metadata().unwrap().len()
		);
		assert_eq!(
			PAGE_SIZE as u64,
			fs::File::open("testdb/rolled/test.1.bc").unwrap().metadata().unwrap().len()
		);
		assert!(fs::File::open("testdb/rolled/test.2.bc").is_err());

		let rolled_file = RolledFile::new("testdb/rolled", "test", "bc", PAGE_SIZE as u64).unwrap();

		assert_eq!(1, rolled_file.read_page(page_one_pref).unwrap().unwrap().read_u64(0));
		assert_eq!(2, rolled_file.read_page(page_one_pref.next_page()).unwrap().unwrap().read_u64(0));
	}

	#[test]
	fn test_rolled_file_truncate() {
		fs::remove_dir_all("testdb/rolled-truncate").unwrap_or_default();

		let mut rolled_file = RolledFile::new("testdb/rolled-truncate", "test", "bc", PAGE_SIZE as u64).unwrap();

		let page_one_pref = PRef::from(0);

		rolled_file.update_page(Page::new_page_with_position(page_one_pref)).unwrap();
		rolled_file
			.update_page(Page::new_page_with_position(page_one_pref.next_page()))
			.unwrap();

		assert_eq!(
			PAGE_SIZE as u64,
			fs::File::open("testdb/rolled-truncate/test.0.bc")
				.unwrap()
				.metadata()
				.unwrap()
				.len()
		);
		assert_eq!(
			PAGE_SIZE as u64,
			fs::File::open("testdb/rolled-truncate/test.1.bc")
				.unwrap()
				.metadata()
				.unwrap()
				.len()
		);

		rolled_file.truncate(1000).unwrap();

		assert_eq!(
			PAGE_SIZE as u64,
			fs::File::open("testdb/rolled-truncate/test.0.bc")
				.unwrap()
				.metadata()
				.unwrap()
				.len()
		);
		assert!(fs::File::open("testdb/rolled-truncate/test.1.bc").is_err());
	}
}
