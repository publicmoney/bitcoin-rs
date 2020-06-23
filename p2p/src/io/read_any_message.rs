use crate::bytes::Bytes;
use crate::io::{read_header, Error, SharedTcpStream};
use crypto::checksum;
use message::{Command, Error as MessageError};
use network::Magic;

pub async fn read_any_message(a: &SharedTcpStream, magic: Magic) -> Result<(Command, Bytes), Error> {
	let header = read_header(&a, magic).await?;

	let mut buf = Bytes::new_with_len(header.len as usize);
	a.read_exact(buf.as_mut()).await?;

	if checksum(&buf) != header.checksum {
		return Err(MessageError::InvalidChecksum.into());
	}
	Ok((header.command.clone(), buf.into()))
}

#[cfg(test)]
mod tests {
	use super::read_any_message;
	use crate::io::shared_tcp_stream::SharedTcpStream;
	use message::Error as MessageError;
	use network::Network;
	use std::error::Error as StdError;

	#[tokio::test]
	async fn test_read_any_message() {
		let stream = SharedTcpStream::new("f9beb4d970696e6700000000000000000800000083c00c765845303b6da97786".into());
		let expected = ("ping".into(), "5845303b6da97786".into());

		assert_eq!(read_any_message(&stream, Network::Mainnet.magic()).await.unwrap(), expected);
	}

	#[tokio::test]
	async fn test_read_any_message_error_wrong_magic() {
		let stream = SharedTcpStream::new("f9beb4d970696e6700000000000000000800000083c00c765845303b6da97786".into());
		let expected_error = MessageError::InvalidMagic;

		assert_eq!(
			expected_error.to_string(),
			read_any_message(&stream, Network::Testnet.magic())
				.await
				.unwrap_err()
				.source()
				.unwrap()
				.to_string()
		);
	}

	#[tokio::test]
	async fn test_read_any_message_error_too_short() {
		let stream = SharedTcpStream::new("f9beb4d970696e6700000000000000000800000083c00c765845303b6da977".into());

		assert!(read_any_message(&stream, Network::Mainnet.magic()).await.is_err());
	}

	#[tokio::test]
	async fn test_read_any_message_error_invalid_checksum() {
		let stream = SharedTcpStream::new("f9beb4d970696e6700000000000000000800000083c01c765845303b6da97786".into());
		let expected_error = MessageError::InvalidChecksum;

		assert_eq!(
			expected_error.to_string(),
			read_any_message(&stream, Network::Mainnet.magic())
				.await
				.unwrap_err()
				.source()
				.unwrap()
				.to_string()
		);
	}
}
