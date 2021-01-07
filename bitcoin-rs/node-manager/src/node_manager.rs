use message::bytes::Bytes;
use message::{serialize_payload, Payload};
use network::network::{PROTOCOL_MINIMUM, PROTOCOL_VERSION};
use network::Network;
use p2p::{connect, read_any_message, Connection, NetConfig};
use rpc_client::{http, RpcClient};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::rc::Rc;
use tokio::time::{Duration, Instant};

pub enum SubCommand {
	Rollback(u32),
	Stats,
	Verify,
}

impl SubCommand {
	fn args(&self) -> Vec<String> {
		match self {
			SubCommand::Rollback(n) => vec!["rollback".to_string(), n.to_string()],
			SubCommand::Stats => vec!["stats".to_string()],
			SubCommand::Verify => vec!["verify".to_string()],
		}
	}
}

pub struct NodeManager {
	process: Option<Child>,
	bin_path: String,
	data_dir: String,
	sub_command: Option<SubCommand>,
	config: NetConfig,
	connection: Option<Connection>,
	rpc_port: String,
	rpc: Option<Rc<RpcClient>>,
}

impl NodeManager {
	pub fn new_test_node(bin_path: &str, node_name: &str, manifest_dir: &str) -> Self {
		let position = std::fs::read_dir(format!("{}/tests", manifest_dir))
			.unwrap()
			.position(|f| f.unwrap().file_name().to_str().unwrap() == format!("{}.rs", node_name))
			.unwrap();

		let port = 10000 + position;
		NodeManager::new_node(bin_path, node_name, port)
	}

	pub fn new_node(bin_path: &str, data_dir: &str, rpc_port: usize) -> Self {
		let data_dir = format!("testdb/{}", data_dir);
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
			rpc_port: rpc_port.to_string(),
			rpc: None,
		}
	}

	pub fn with_sub_command(&mut self, sub_command: SubCommand) -> &mut NodeManager {
		self.sub_command = Some(sub_command);
		self
	}

	pub fn start(&mut self) -> &mut NodeManager {
		let mut bitcoin_rs_cmd = Command::new(&self.bin_path);

		bitcoin_rs_cmd
			.args(&["--regtest", "--data-dir", &self.data_dir, "--jsonrpc-port", &self.rpc_port])
			.stdout(Stdio::null());

		if let Some(sub_command) = &self.sub_command {
			bitcoin_rs_cmd.args(sub_command.args());
			self.sub_command = None;
		};

		let bitcoin_rs = bitcoin_rs_cmd.spawn().expect("Error starting node");

		self.process = Some(bitcoin_rs);
		self
	}

	pub async fn connect_rpc(&mut self) -> &mut NodeManager {
		let client = http::connect::<RpcClient>(&format!("http://127.0.0.1:{}", &self.rpc_port))
			.await
			.unwrap();
		self.rpc = Some(Rc::new(client));
		for _ in 0..5 {
			if let Ok(_) = self.rpc().memory_info().await {
				return self;
			}
			tokio::time::sleep(std::time::Duration::from_secs(1)).await;
		}
		panic!("RPC connection failed")
	}

	pub fn rpc(&self) -> Rc<RpcClient> {
		self.rpc.clone().expect("RPC client not connected")
	}

	pub async fn connect_p2p(&mut self) -> &mut NodeManager {
		let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), self.config.network.port());
		for _ in 0..5 {
			if let Ok(connection) = connect(&socket, &self.config).await {
				self.connection = Some(connection);
				return self;
			}
			tokio::time::sleep(std::time::Duration::from_secs(1)).await;
		}
		panic!("P2P connection failed")
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

	pub async fn wait_for_exit(&mut self, max_duration: Duration) -> Result<ExitStatus, String> {
		let start = Instant::now();
		if let Some(process) = self.process.as_mut() {
			while Instant::now().duration_since(start) < max_duration {
				match process.try_wait() {
					Ok(Some(status)) => {
						self.process = None;
						return Ok(status);
					}
					Err(e) => return Err(e.to_string()),
					_ => {}
				}
				tokio::time::sleep(Duration::from_secs(1)).await;
			}
			Err("Timed out waiting for exit.".to_string())
		} else {
			Err("Process already terminated.".to_string())
		}
	}

	pub fn kill(&mut self) {
		if let Some(mut process) = self.process.take() {
			process.kill().unwrap_or_default();
			process.wait().unwrap();
			self.process = None
		}
	}
}

impl Drop for NodeManager {
	fn drop(&mut self) {
		self.kill()
	}
}
