use message::bytes::Bytes;
use message::{serialize_payload, Payload};
use network::network::{PROTOCOL_MINIMUM, PROTOCOL_VERSION};
use network::Network;
use p2p::{connect, read_any_message, Connection, NetConfig};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::process::{Child, Command, ExitStatus, Stdio};

pub struct NodeManager {
	process: Option<Child>,
	bin_path: String,
	data_dir: String,
	sub_command: Option<String>,
	config: NetConfig,
	connection: Option<Connection>,
}

impl NodeManager {
	pub fn new_node(bin_path: &str, data_dir: &str) -> Self {
		let data_dir = "testdb".to_string() + "/" + data_dir;
		std::fs::remove_dir_all(&data_dir).unwrap_or_default();

		let config = NetConfig {
			protocol_version: PROTOCOL_VERSION,
			protocol_minimum: PROTOCOL_MINIMUM,
			network: Network::Regtest,
			local_address: "0.0.0.0:3000".parse().unwrap(),
			services: Default::default(),
			user_agent: "bitcoin-rs-test".to_string(),
			start_height: 0,
			relay: false,
		};

		NodeManager {
			process: None,
			bin_path: bin_path.to_string(),
			data_dir: data_dir.to_string(),
			sub_command: None,
			config,
			connection: None,
		}
	}

	pub fn with_sub_command(mut self, sub_command: &str) -> NodeManager {
		self.sub_command = Some(sub_command.to_string());
		self
	}

	pub fn start(mut self) -> NodeManager {
		let mut bitcoin_rs_cmd = Command::new(&self.bin_path);

		bitcoin_rs_cmd
			.args(&["--regtest", "--data-dir", &self.data_dir])
			.stdout(Stdio::null());

		if let Some(sub_command) = &self.sub_command {
			bitcoin_rs_cmd.arg(sub_command);
		};

		let bitcoin_rs = bitcoin_rs_cmd.spawn().expect("Error starting node");

		self.process = Some(bitcoin_rs);
		self
	}

	pub async fn connect_p2p(mut self) -> NodeManager {
		let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), self.config.network.port());
		for _ in 0..5 {
			if let Ok(connection) = connect(&socket, &self.config).await {
				self.connection = Some(connection);
				return self;
			}
			tokio::time::sleep(std::time::Duration::from_secs(1)).await;
		}
		panic!("unable to connect to node")
	}

	pub async fn send_message<T>(&self, payload: &T) -> Result<(), String>
	where
		T: Payload,
	{
		if let Some(connection) = &self.connection {
			let stream = serialize_payload(payload, self.config.protocol_version).unwrap();
			connection.stream.write_all(stream.as_ref()).await.unwrap();
			Ok(())
		} else {
			Err("Not connected".to_string())
		}
	}

	pub async fn read_message(&self) -> Result<(message::Command, Bytes), String> {
		if let Some(connection) = &self.connection {
			Ok(read_any_message(&connection.stream, self.config.network.magic()).await.unwrap())
		} else {
			Err("Not connected".to_string())
		}
	}

	pub fn wait_for_exit(mut self) -> Option<ExitStatus> {
		if let Some(process) = self.process.as_mut() {
			return process.wait().ok();
		}
		None
	}

	pub fn stop(&mut self) {
		if let Some(process) = self.process.as_mut() {
			process.kill().unwrap_or_default();
			process.wait().unwrap();
			self.process = None
		}
	}
}

impl Drop for NodeManager {
	fn drop(&mut self) {
		self.stop()
	}
}
