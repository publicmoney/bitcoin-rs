// use crate::stream::{Serializable, Stream};
// use crate::reader::{Reader, Deserializable, Error};

#[macro_export]
macro_rules! impl_ser_for_array {
	($name: ident, $size: expr) => {
		impl Serializable for $name {
			fn serialize(&self, stream: &mut Stream) {
				stream.append_slice(&**self);
			}

			#[inline]
			fn serialized_size(&self) -> usize {
				$size
			}
		}

		impl Deserializable for $name {
			fn deserialize<T>(reader: &mut Reader<T>) -> Result<Self, Error>
			where
				T: std::io::Read,
			{
				let mut result = Self::default();
				reader.read_slice(&mut *result)?;
				Ok(result)
			}
		}
	};
}

#[macro_export]
macro_rules! impl_ser_for_hash {
	($name: ident, $size: expr) => {
		use bitcrypto::Hash;
		impl Serializable for $name {
			fn serialize(&self, stream: &mut Stream) {
				stream.append_slice(&**self);
			}

			#[inline]
			fn serialized_size(&self) -> usize {
				$size
			}
		}

		impl Deserializable for $name {
			fn deserialize<T>(reader: &mut Reader<T>) -> Result<Self, Error>
			where
				T: io::Read,
			{
				let mut s = [0; $size];
				reader.read_slice(&mut s)?;
				Self::from_slice(&s).map_err(|_| Error::MalformedData)
			}
		}
	};
}
