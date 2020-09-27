use message::bytes::Bytes;
use message::{serialize_payload, Payload};
use network::network::{PROTOCOL_MINIMUM, PROTOCOL_VERSION};
use network::Network;
use p2p::{connect, read_any_message, Connection, NetConfig};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::process::{Child, Command, ExitStatus, Stdio};

pub struct NodeManager {
	builder: NodeBuilder,
	process: Option<Child>,
	connection: Option<Connection>,
}

pub struct NodeBuilder {
	bin_path: String,
	config: NetConfig,
	sub_command: Option<String>,
}

impl NodeBuilder {
	pub fn new(bin_path: &str) -> Self {
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
		NodeBuilder {
			bin_path: bin_path.to_string(),
			config,
			sub_command: None,
		}
	}

	pub fn with_sub_command(&mut self, sub_command: String) -> &mut NodeBuilder {
		self.sub_command = Some(sub_command);
		self
	}
}

impl NodeManager {
	pub fn new_node(bin_path: &str) -> Self {
		std::fs::remove_dir_all("testdb").unwrap_or_default();

		NodeManager {
			builder: NodeBuilder::new(bin_path),
			process: None,
			connection: None,
		}
	}

	pub fn builder(&mut self) -> &mut NodeBuilder {
		&mut self.builder
	}

	pub fn start(&mut self) -> &mut NodeManager {
		let mut bitcoin_rs_cmd = Command::new(&self.builder.bin_path);

		bitcoin_rs_cmd
			.arg("--regtest")
			.args(&["--data-dir", "testdb"])
			.stdout(Stdio::null());

		if let Some(sub_command) = &self.builder.sub_command {
			bitcoin_rs_cmd.arg(sub_command);
		};

		let bitcoin_rs = bitcoin_rs_cmd.spawn().expect("Error starting node");

		self.process = Some(bitcoin_rs);
		self
	}

	pub async fn connect_p2p(&mut self) -> &mut NodeManager {
		let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), self.builder.config.network.port());
		for _ in 0..5 {
			if let Ok(connection) = connect(&socket, &self.builder.config).await {
				self.connection = Some(connection);
				return self;
			}
			std::thread::sleep(std::time::Duration::from_secs(1));
		}
		panic!("unable to connect to node")
	}

	pub async fn send_message<T>(&self, payload: &T) -> Result<(), String>
	where
		T: Payload,
	{
		if let Some(connection) = &self.connection {
			let stream = serialize_payload(payload, self.builder.config.protocol_version).unwrap();
			connection.stream.write_all(stream.as_ref()).await.unwrap();
			Ok(())
		} else {
			Err("Not connected".to_string())
		}
	}

	pub async fn read_message(&self) -> Result<(message::Command, Bytes), String> {
		if let Some(connection) = &self.connection {
			Ok(read_any_message(&connection.stream, self.builder.config.network.magic())
				.await
				.unwrap())
		} else {
			Err("Not connected".to_string())
		}
	}

	pub fn wait_for_exit(&mut self) -> Option<ExitStatus> {
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
