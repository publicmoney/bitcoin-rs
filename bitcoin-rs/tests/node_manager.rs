use message::bytes::Bytes;
use message::{serialize_payload, Payload};
use network::network::{PROTOCOL_MINIMUM, PROTOCOL_VERSION};
use network::Network;
use p2p::{connect, read_any_message, Connection, NetConfig};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::process::{Child, Command, Stdio};

pub struct NodeManager {
	process: Child,
	connection: Connection,
	config: NetConfig,
}

impl NodeManager {
	pub async fn new_node() -> Self {
		std::fs::remove_dir_all("testdb").unwrap_or_default();

		let bitcoin_rs_cmd = env!("CARGO_BIN_EXE_bitcoin-rs");

		let bitcoin_rs = Command::new(bitcoin_rs_cmd)
			.arg("--regtest")
			.args(&["--data-dir", "testdb"])
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()
			.expect("failed to start bitcoin-rs");

		let network = Network::Regtest;
		let config = NetConfig {
			protocol_version: PROTOCOL_VERSION,
			protocol_minimum: PROTOCOL_MINIMUM,
			magic: network.magic(),
			local_address: "0.0.0.0:3000".parse().unwrap(),
			services: Default::default(),
			user_agent: "bitcoin-rs-test".to_string(),
			start_height: 0,
			relay: false,
		};

		std::thread::sleep(std::time::Duration::from_secs(1));
		let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), network.port());
		let connection = connect(&socket, &config).await.unwrap();

		NodeManager {
			process: bitcoin_rs,
			connection,
			config,
		}
	}

	pub async fn send_message<T>(&self, payload: &T)
	where
		T: Payload,
	{
		let stream = serialize_payload(payload, self.config.protocol_version).unwrap();
		self.connection.stream.write_all(stream.as_ref()).await.unwrap();
	}

	pub async fn read_message(&self) -> (message::Command, Bytes) {
		read_any_message(&self.connection.stream, self.config.magic).await.unwrap()
	}

	pub fn stop(&mut self) {
		self.process.kill().unwrap_or_default()
	}
}

impl Drop for NodeManager {
	fn drop(&mut self) {
		self.stop()
	}
}
