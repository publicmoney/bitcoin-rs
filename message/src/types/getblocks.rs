use crate::{MessageResult, Payload};
use bitcrypto::SHA256D;
use ser::{Reader, Stream};
use std::io;

pub const GETBLOCKS_MAX_RESPONSE_HASHES: usize = 500;

#[derive(Debug, PartialEq)]
pub struct GetBlocks {
	pub version: u32,
	pub block_locator_hashes: Vec<SHA256D>,
	pub hash_stop: SHA256D,
}

impl Payload for GetBlocks {
	fn version() -> u32 {
		0
	}

	fn command() -> &'static str {
		"getblocks"
	}

	fn deserialize_payload<T>(reader: &mut Reader<T>, _version: u32) -> MessageResult<Self>
	where
		T: io::Read,
	{
		let get_blocks = GetBlocks {
			version: reader.read()?,
			block_locator_hashes: reader.read_list_max(500)?,
			hash_stop: reader.read()?,
		};

		Ok(get_blocks)
	}

	fn serialize_payload(&self, stream: &mut Stream, _version: u32) -> MessageResult<()> {
		stream
			.append(&self.version)
			.append_list::<SHA256D, SHA256D>(&self.block_locator_hashes)
			.append(&self.hash_stop);
		Ok(())
	}
}
