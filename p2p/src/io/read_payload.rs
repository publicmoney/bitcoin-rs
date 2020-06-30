use crate::bytes::Bytes;
use crate::io::{Error, SharedTcpStream};
use message::{deserialize_payload, Error as MessageError, Payload};
use primitives::checksum::Checksum;

pub async fn read_payload<M>(a: &SharedTcpStream, version: u32, len: usize, checksum: Checksum) -> Result<M, Error>
where
	M: Payload,
{
	let mut buf = Bytes::new_with_len(len);
	a.read_exact(buf.as_mut()).await?;
	if Checksum::generate(&buf) != checksum {
		return Err(MessageError::InvalidChecksum.into());
	}
	deserialize_payload(&buf, version).map_err(|e| e.into())
}

#[cfg(test)]
mod tests {
	use super::read_payload;
	use crate::io::shared_tcp_stream::SharedTcpStream;
	use message::types::Ping;
	use message::Error as MessageError;
	use std::error::Error;

	#[tokio::test]
	async fn test_read_payload() {
		let stream = SharedTcpStream::new("5845303b6da97786".into());
		let ping = Ping::new(u64::from_str_radix("8677a96d3b304558", 16).unwrap());

		assert_eq!(read_payload::<Ping>(&stream, 0, 8, "83c00c76".into()).await.unwrap(), ping);
	}

	#[tokio::test]
	async fn test_read_payload_with_invalid_checksum() {
		let stream = SharedTcpStream::new("5845303b6da97786".into());
		let expected_err = MessageError::InvalidChecksum;

		assert_eq!(
			expected_err.to_string(),
			read_payload::<Ping>(&stream, 0, 8, "83c00c75".into())
				.await
				.unwrap_err()
				.source()
				.unwrap()
				.to_string()
		);
	}

	#[tokio::test]
	async fn test_read_too_short_payload() {
		let stream = SharedTcpStream::new("5845303b6da977".into());

		assert!(read_payload::<Ping>(&stream, 0, 8, "83c00c76".into()).await.is_err());
	}
}
