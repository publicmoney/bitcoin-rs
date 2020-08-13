use bitcrypto::SHA256D;

#[derive(Debug, Clone, Copy)]
pub enum BlockRef {
	Number(u32),
	Hash(SHA256D),
}

impl From<u32> for BlockRef {
	fn from(u: u32) -> Self {
		BlockRef::Number(u)
	}
}

impl From<SHA256D> for BlockRef {
	fn from(hash: SHA256D) -> Self {
		BlockRef::Hash(hash)
	}
}
