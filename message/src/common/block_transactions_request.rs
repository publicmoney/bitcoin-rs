use crate::hash::H256;
use ser::{CompactInteger, Deserializable, Error as ReaderError, Reader, Serializable, Stream};
use std::io;

#[derive(Debug, PartialEq)]
pub struct BlockTransactionsRequest {
	pub blockhash: H256,
	pub indexes: Vec<usize>,
}

impl Serializable for BlockTransactionsRequest {
	fn serialize(&self, stream: &mut Stream) {
		let indexes: Vec<CompactInteger> = self.indexes.iter().map(|x| (*x).into()).collect();

		stream.append(&self.blockhash).append_list(&indexes);
	}
}

impl Deserializable for BlockTransactionsRequest {
	fn deserialize<T>(reader: &mut Reader<T>) -> Result<Self, ReaderError>
	where
		T: io::Read,
	{
		let blockhash = reader.read()?;
		let indexes: Vec<CompactInteger> = reader.read_list()?;

		let request = BlockTransactionsRequest {
			blockhash,
			indexes: indexes.into_iter().map(Into::into).collect(),
		};

		Ok(request)
	}
}
