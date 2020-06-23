use crate::io::{Error, SharedTcpStream};
use message::MessageHeader;
use network::Magic;

pub async fn read_header(a: &SharedTcpStream, magic: Magic) -> Result<MessageHeader, Error> {
	let mut buf = [0u8; 24];
	a.read_exact(&mut buf).await?;
	MessageHeader::deserialize(&buf, magic).map_err(|e| e.into())
}

#[cfg(test)]
mod tests {
	use super::read_header;
	use crate::io::shared_tcp_stream::SharedTcpStream;
	use message::{Error as MessageError, MessageHeader};
	use network::Network;
	use std::error::Error;

	#[tokio::test]
	async fn test_read_header() {
		let stream = SharedTcpStream::new("f9beb4d96164647200000000000000001f000000ed52399b".into());
		let expected = MessageHeader {
			magic: Network::Mainnet.magic(),
			command: "addr".into(),
			len: 0x1f,
			checksum: "ed52399b".into(),
		};

		assert_eq!(read_header(&stream, Network::Mainnet.magic()).await.unwrap(), expected);
	}

	#[tokio::test]
	async fn test_read_header_error_invalid_magic() {
		let stream = SharedTcpStream::new("f9beb4d96164647200000000000000001f000000ed52399b".into());
		let expected_error = MessageError::InvalidMagic;

		assert_eq!(
			expected_error.to_string(),
			read_header(&stream, Network::Testnet.magic())
				.await
				.unwrap_err()
				.source()
				.unwrap()
				.to_string()
		);
	}

	#[tokio::test]
	async fn test_read_header_error_too_short() {
		let stream = SharedTcpStream::new("f9beb4d96164647200000000000000001f000000ed5239".into());
		assert!(read_header(&stream, Network::Mainnet.magic()).await.is_err());
	}
}
