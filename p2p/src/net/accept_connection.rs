use crate::io::{accept_handshake, Error, SharedTcpStream};
use crate::net::{Config, Connection};
use std::net;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;

pub async fn accept_connection<'a>(stream: TcpStream, config: &Config, address: net::SocketAddr) -> Result<Connection, Error> {
	let shared_stream: SharedTcpStream = stream.into();
	let handshake = async {
		let handshake_result = accept_handshake(
			&shared_stream,
			config.network.magic(),
			config.version(&address),
			config.protocol_minimum,
		)
		.await?;

		Ok(Connection {
			stream: shared_stream,
			services: handshake_result.version.services(),
			version: handshake_result.negotiated_version,
			version_message: handshake_result.version,
			magic: config.network.magic(),
			address,
		})
	};

	timeout(Duration::new(5, 0), handshake).await?
}
