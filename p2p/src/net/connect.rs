use crate::io::{handshake, Error, SharedTcpStream};
use crate::net::{Config, Connection};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::timeout;

pub async fn connect<'a>(address: &SocketAddr, config: &Config) -> Result<Connection, Error> {
	let stream = SharedTcpStream::connect(address).await?;
	let connect = async {
		let handshake = handshake(&stream, config.magic, config.version(address), config.protocol_minimum).await?;

		Ok(Connection {
			stream,
			services: handshake.version.services(),
			version: handshake.negotiated_version,
			version_message: handshake.version,
			magic: config.magic,
			address: *address,
		})
	};

	timeout(Duration::new(5, 0), connect).await?
}
