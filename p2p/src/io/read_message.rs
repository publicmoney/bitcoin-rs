use crate::io::{read_header, read_payload, Error, SharedTcpStream};
use message::{Error as MessageError, Payload};
use network::Magic;

pub async fn read_message<M>(a: &SharedTcpStream, magic: Magic, version: u32) -> Result<M, Error>
where
	M: Payload,
{
	let header = read_header(a, magic).await?;

	if header.command != M::command() {
		return Err(MessageError::InvalidCommand.into());
	}
	read_payload(a, version, header.len as usize, header.checksum).await
}

#[cfg(test)]
mod tests {
	use super::read_message;
	use crate::io::shared_tcp_stream::SharedTcpStream;
	use message::types::{Ping, Pong};
	use message::Error as MessageError;
	use network::Network;
	use std::error::Error;

	#[tokio::test]
	async fn test_read_message() {
		let stream = SharedTcpStream::new("f9beb4d970696e6700000000000000000800000083c00c765845303b6da97786".into());
		let ping = Ping::new(u64::from_str_radix("8677a96d3b304558", 16).unwrap());

		assert_eq!(read_message::<Ping>(&stream, Network::Mainnet.magic(), 0).await.unwrap(), ping);
	}

	#[tokio::test]
	async fn test_read_message_error_invalid_magic() {
		let stream = SharedTcpStream::new("f9beb4d970696e6700000000000000000800000083c00c765845303b6da97786".into());
		let expected_error = MessageError::InvalidMagic;

		assert_eq!(
			expected_error.to_string(),
			read_message::<Ping>(&stream, Network::Testnet.magic(), 0)
				.await
				.unwrap_err()
				.source()
				.unwrap()
				.to_string()
		);
	}

	#[tokio::test]
	async fn test_read_message_error_invalid_command() {
		let stream = SharedTcpStream::new("f9beb4d970696e6700000000000000000800000083c00c765845303b6da97786".into());
		let expected_error = MessageError::InvalidCommand;

		assert_eq!(
			expected_error.to_string(),
			read_message::<Pong>(&stream, Network::Mainnet.magic(), 0)
				.await
				.unwrap_err()
				.source()
				.unwrap()
				.to_string()
		);
	}

	#[tokio::test]
	async fn test_read_too_short_message() {
		let stream = SharedTcpStream::new("f9beb4d970696e6700000000000000000800000083c00c765845303b6da977".into());

		assert!(read_message::<Ping>(&stream, Network::Mainnet.magic(), 0).await.is_err());
	}

	#[tokio::test]
	async fn test_read_message_with_invalid_checksum() {
		let stream = SharedTcpStream::new("f9beb4d970696e6700000000000000000800000083c01c765845303b6da97786".into());
		let expected_error = MessageError::InvalidChecksum;

		assert_eq!(
			expected_error.to_string(),
			read_message::<Ping>(&stream, Network::Mainnet.magic(), 0)
				.await
				.unwrap_err()
				.source()
				.unwrap()
				.to_string()
		);
	}
}
