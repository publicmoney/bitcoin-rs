use bitcrypto::{Hash, HashEngine, SHA256D};
use ser::{Deserializable, Error as ReaderError, Reader};
use std::io;

pub struct HashedData<T> {
	pub size: usize,
	pub hash: SHA256D,
	pub data: T,
}

pub trait ReadAndHash {
	fn read_and_hash<T>(&mut self) -> Result<HashedData<T>, ReaderError>
	where
		T: Deserializable;
}

impl<R> ReadAndHash for Reader<R>
where
	R: io::Read,
{
	fn read_and_hash<T>(&mut self) -> Result<HashedData<T>, ReaderError>
	where
		T: Deserializable,
	{
		let mut size = 0usize;
		let mut engine = SHA256D::engine();
		let data = self.read_with_proxy(|bytes| {
			size += bytes.len();
			engine.input(bytes);
		})?;

		let result = HashedData {
			hash: SHA256D::from_engine(engine),
			data,
			size,
		};

		Ok(result)
	}
}
