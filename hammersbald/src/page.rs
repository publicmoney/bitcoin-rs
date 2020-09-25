use crate::pref::{PRef, PREF_SIZE};
use byteorder::{BigEndian, ByteOrder};

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_PAYLOAD_SIZE: usize = PAGE_SIZE - PREF_SIZE;

/// A page of the persistent files
#[derive(Clone)]
pub struct Page {
	content: [u8; PAGE_SIZE],
}

impl Page {
	/// create an empty page for a position in the data/table file (updatable).
	pub fn new_page_with_position(pref: PRef) -> Page {
		let mut page = Page { content: [0u8; PAGE_SIZE] };
		page.write_pref(PAGE_PAYLOAD_SIZE, pref);
		page
	}

	/// create an empty page for log file (not updatable)
	pub fn new() -> Page {
		Page { content: [0u8; PAGE_SIZE] }
	}

	/// create a Page from read buffer
	pub fn from_buf(content: [u8; PAGE_SIZE]) -> Page {
		Page { content }
	}

	/// interpret the last 6 bytes as an pref
	pub fn pref(&self) -> PRef {
		self.read_pref(PAGE_PAYLOAD_SIZE)
	}

	/// write slice at a position
	pub fn write(&mut self, pos: usize, slice: &[u8]) {
		self.content[pos..pos + slice.len()].copy_from_slice(slice)
	}

	/// read at position
	pub fn read(&self, pos: usize, buf: &mut [u8]) {
		let len = buf.len();
		buf.copy_from_slice(&self.content[pos..pos + len])
	}

	/// write a pref into the page
	pub fn write_pref(&mut self, pos: usize, pref: PRef) {
		let mut buf = [0u8; PREF_SIZE];
		BigEndian::write_u48(&mut buf, pref.as_u64());
		self.content[pos..pos + PREF_SIZE].copy_from_slice(&buf[..]);
	}

	/// read a pref at a page position
	pub fn read_pref(&self, pos: usize) -> PRef {
		PRef::from(BigEndian::read_u48(&self.content[pos..pos + PREF_SIZE]))
	}

	/// write u64 into the page
	pub fn write_u64(&mut self, pos: usize, n: u64) {
		let mut buf = [0u8; 8];
		BigEndian::write_u64(&mut buf, n);
		self.content[pos..pos + 8].copy_from_slice(&buf[..]);
	}

	/// read a u64 at a page position
	pub fn read_u64(&self, pos: usize) -> u64 {
		BigEndian::read_u64(&self.content[pos..pos + 8])
	}

	/// into write buffer
	pub fn into_buf(self) -> [u8; PAGE_SIZE] {
		self.content
	}
}

#[test]
fn test_page_with_position() {
	let pref = PRef::from(5);
	let page = Page::new_page_with_position(pref);
	let mut result = [0u8; PREF_SIZE];
	page.read(PAGE_PAYLOAD_SIZE, &mut result);
	assert_eq!(pref, page.pref());
	assert_eq!([0, 0, 0, 0, 0, 5], result)
}

#[test]
fn test_read_write() {
	let mut page = Page::new();
	let data = [1, 2, 3];
	page.write(10, &data);

	let mut result = [0u8; 3];
	page.read(10, &mut result);
	assert_eq!(data, result)
}
