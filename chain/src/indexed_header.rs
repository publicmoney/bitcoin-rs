use crate::block_header::{block_header_hash, BlockHeader};
use crate::read_and_hash::ReadAndHash;
use bitcrypto::SHA256D;
use ser::{Deserializable, Error as ReaderError, Reader};
use std::{cmp, fmt, io};

#[derive(Clone)]
pub struct IndexedBlockHeader {
	pub hash: SHA256D,
	pub raw: BlockHeader,
}

impl fmt::Debug for IndexedBlockHeader {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		f.debug_struct("IndexedBlockHeader")
			.field("hash", &self.hash)
			.field("raw", &self.raw)
			.finish()
	}
}

#[cfg(feature = "test-helpers")]
impl From<BlockHeader> for IndexedBlockHeader {
	fn from(header: BlockHeader) -> Self {
		Self::from_raw(header)
	}
}
impl IndexedBlockHeader {
	pub fn new(hash: SHA256D, header: BlockHeader) -> Self {
		IndexedBlockHeader { hash, raw: header }
	}

	/// Explicit conversion of the raw BlockHeader into IndexedBlockHeader.
	///
	/// Hashes the contents of block header.
	pub fn from_raw(header: BlockHeader) -> Self {
		IndexedBlockHeader::new(block_header_hash(&header), header)
	}
}

impl cmp::PartialEq for IndexedBlockHeader {
	fn eq(&self, other: &Self) -> bool {
		self.hash == other.hash
	}
}

impl Deserializable for IndexedBlockHeader {
	fn deserialize<T>(reader: &mut Reader<T>) -> Result<Self, ReaderError>
	where
		T: io::Read,
	{
		let data = reader.read_and_hash::<BlockHeader>()?;
		// TODO: use len
		let header = IndexedBlockHeader {
			raw: data.data,
			hash: data.hash,
		};

		Ok(header)
	}
}
