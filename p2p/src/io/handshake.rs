use crate::io::Error;
use crate::io::{read_message, write_message, SharedTcpStream};
use message::types::{Verack, Version};
use message::{Error as MessageError, Message};
use network::Magic;
use std::cmp;

pub async fn handshake(a: &SharedTcpStream, magic: Magic, version: Version, min_version: u32) -> Result<HandshakeResult, Error> {
	write_message(a, version_message(magic, &version)).await?;

	let peer_version: Version = read_message(a, magic, 0).await?;

	if peer_version.version() < min_version {
		return Err(MessageError::InvalidVersion.into());
	}
	if let (Some(self_nonce), Some(peer_nonce)) = (version.nonce(), peer_version.nonce()) {
		if self_nonce == peer_nonce {
			return Err(MessageError::InvalidVersion.into());
		}
	}
	write_message(a, verack_message(magic)).await?;

	let _: Verack = read_message(a, magic, 0).await?;

	Ok(HandshakeResult {
		negotiated_version: negotiate_version(version.version(), peer_version.version()),
		version: peer_version,
	})
}

pub async fn accept_handshake(a: &SharedTcpStream, magic: Magic, version: Version, min_version: u32) -> Result<HandshakeResult, Error> {
	let peer_version: Version = read_message(a, magic, 0).await?;

	if peer_version.version() < min_version {
		return Err(MessageError::InvalidVersion.into());
	}
	if let (Some(self_nonce), Some(nonce)) = (version.nonce(), peer_version.nonce()) {
		if self_nonce == nonce {
			return Err(MessageError::InvalidVersion.into());
		}
	}
	write_message(a, version_message(magic, &version)).await?;
	write_message(a, verack_message(magic)).await?;

	Ok(HandshakeResult {
		negotiated_version: negotiate_version(version.version(), peer_version.version()),
		version: peer_version,
	})
}

pub fn negotiate_version(local: u32, other: u32) -> u32 {
	cmp::min(local, other)
}

#[derive(Debug, PartialEq)]
pub struct HandshakeResult {
	pub version: Version,
	pub negotiated_version: u32,
}

fn version_message(magic: Magic, version: &Version) -> Message<Version> {
	Message::new(magic, version.version(), version).expect("version message should always be serialized correctly")
}

fn verack_message(magic: Magic) -> Message<Verack> {
	Message::new(magic, 0, &Verack).expect("verack message should always be serialized correctly")
}

#[cfg(test)]
mod tests {
	use super::{accept_handshake, handshake, HandshakeResult};
	use crate::io::shared_tcp_stream::SharedTcpStream;
	use message::types::version::{Version, V0, V106, V70001};
	use message::types::Verack;
	use message::{Error as MessageError, Message};
	use network::Network;
	use ser::Stream;
	use std::error::Error;

	fn local_version() -> Version {
		Version::V70001(
			V0 {
				version: 70001,
				services: 1u64.into(),
				timestamp: 0x4d1015e6,
				// address and port of remote
				// services set to 0, cause we know nothing about the node
				receiver: "00000000000000000000000000000000000000002f5a0808208d".into(),
			},
			V106 {
				// our local address (not sure if it is valid, or if it is checked at all
				// services set to 0, because we support nothing
				from: "00000000000000000000000000000000000000007f000001208d".into(),
				nonce: 0x3c76a409eb48a227,
				user_agent: "bitcoin-rs".into(),
				start_height: 0,
			},
			V70001 { relay: true },
		)
	}

	fn remote_version() -> Version {
		Version::V70001(
			V0 {
				version: 70012,
				services: 1u64.into(),
				timestamp: 0x4d1015e6,
				// services set to 1, house receiver supports at least the network
				receiver: "010000000000000000000000000000000000ffffc2b5936adde9".into(),
			},
			V106 {
				// remote address, port
				// and supported protocols
				from: "050000000000000000000000000000000000ffff2f5a0808208d".into(),
				nonce: 0x3c76a409eb48a228,
				user_agent: "/Satoshi:0.12.1/".into(),
				start_height: 0,
			},
			V70001 { relay: true },
		)
	}

	#[tokio::test]
	async fn test_handshake() {
		let magic = Network::Mainnet.magic();
		let local_version = local_version();
		let remote_version = remote_version();

		let mut remote_stream = Stream::new();
		remote_stream.append_slice(Message::new(magic, remote_version.version(), &remote_version).unwrap().as_ref());
		remote_stream.append_slice(Message::new(magic, remote_version.version(), &Verack).unwrap().as_ref());
		let shared_stream = SharedTcpStream::new(remote_stream.out());

		let mut expected_stream = Stream::new();
		expected_stream.append_slice(Message::new(magic, remote_version.version(), &local_version).unwrap().as_ref());
		expected_stream.append_slice(Message::new(magic, remote_version.version(), &Verack).unwrap().as_ref());

		let expected = HandshakeResult {
			version: remote_version,
			negotiated_version: 70001,
		};

		let hs = handshake(&shared_stream, magic, local_version, 0).await;
		assert_eq!(shared_stream.get_written().await, expected_stream.out());
		assert_eq!(hs.unwrap(), expected);
	}

	#[tokio::test]
	async fn test_accept_handshake() {
		let magic = Network::Mainnet.magic();
		let local_version = local_version();
		let remote_version = remote_version();

		let mut remote_stream = Stream::new();
		remote_stream.append_slice(Message::new(magic, remote_version.version(), &remote_version).unwrap().as_ref());
		let shared_stream = SharedTcpStream::new(remote_stream.out());

		let mut expected_stream = Stream::new();
		expected_stream.append_slice(Message::new(magic, local_version.version(), &local_version).unwrap().as_ref());
		expected_stream.append_slice(Message::new(magic, local_version.version(), &Verack).unwrap().as_ref());

		let expected = HandshakeResult {
			version: remote_version,
			negotiated_version: 70001,
		};

		let hs = accept_handshake(&shared_stream, magic, local_version, 0).await;
		assert_eq!(shared_stream.get_written().await, expected_stream.out());
		assert_eq!(hs.unwrap(), expected);
	}

	#[tokio::test]
	async fn test_self_handshake() {
		let magic = Network::Mainnet.magic();
		let remote_version = local_version();
		let local_version = local_version();

		let mut remote_stream = Stream::new();
		remote_stream.append_slice(Message::new(magic, remote_version.version(), &remote_version).unwrap().as_ref());
		let shared_stream = SharedTcpStream::new(remote_stream.out());

		let expected_error = MessageError::InvalidVersion;

		let hs = handshake(&shared_stream, magic, local_version, 0).await;
		assert_eq!(expected_error.to_string(), hs.unwrap_err().source().unwrap().to_string());
	}

	#[tokio::test]
	async fn test_accept_self_handshake() {
		let magic = Network::Mainnet.magic();
		let remote_version = local_version();
		let local_version = local_version();

		let mut remote_stream = Stream::new();
		remote_stream.append_slice(Message::new(magic, remote_version.version(), &remote_version).unwrap().as_ref());
		let shared_stream = SharedTcpStream::new(remote_stream.out());

		let expected_error = MessageError::InvalidVersion;

		let hs = accept_handshake(&shared_stream, magic, local_version, 0).await;
		assert_eq!(expected_error.to_string(), hs.unwrap_err().source().unwrap().to_string());
	}
}
