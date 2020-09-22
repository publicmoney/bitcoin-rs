use super::super::rpc;
use crate::config;
use crate::util::{db_path, node_table_path};
use bitcrypto::SHA256D;
use memory::Memory;
use network::network::{PROTOCOL_MINIMUM, PROTOCOL_VERSION};
use p2p::LocalSyncNodeRef;
use std::net::SocketAddr;
use std::process::ExitStatus;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use storage::SharedStore;
use sync::{create_local_sync_node, create_sync_connection_factory, create_sync_peers, LocalNodeRef, SyncListener};
use tokio::io::Error;
use tokio::runtime::Runtime;

enum BlockNotifierTask {
	NewBlock(SHA256D),
	Stop,
}

struct BlockNotifier {
	tx: Sender<BlockNotifierTask>,
	is_synchronizing: Arc<AtomicBool>,
	worker_thread: Option<thread::JoinHandle<()>>,
}

impl BlockNotifier {
	pub fn new(block_notify_command: String) -> Self {
		let (tx, rx) = channel();
		let is_synchronizing = Arc::new(AtomicBool::default());
		BlockNotifier {
			tx,
			is_synchronizing: is_synchronizing.clone(),
			worker_thread: Some(
				thread::Builder::new()
					.name("Block notification thread".to_owned())
					.spawn(move || BlockNotifier::worker(rx, block_notify_command))
					.expect("Error creating block notification thread"),
			),
		}
	}

	fn worker(rx: Receiver<BlockNotifierTask>, block_notify_command: String) {
		for cmd in rx {
			match cmd {
				BlockNotifierTask::NewBlock(new_block_hash) => {
					let new_block_hash = std::str::from_utf8(&new_block_hash[..]).expect("Error parsing block hash for notify command");
					let command = block_notify_command.replace("%s", new_block_hash);

					match std::process::Command::new(command).status() {
						Ok(status) => {
							if !status.success() {
								match status.code() {
									Some(code) => {
										error!(target: "bitcoin-rs", "Block notification command {} exited with error code {}", command, code)
									}
									None => error!(target: "bitcoin-rs", "Block notification command {} terminated by signal", command),
								}
							}
						}
						Err(err) => error!(target: "bitcoin-rs", "Block notification command {} exited with error code {}", command, err),
					}
				}
				BlockNotifierTask::Stop => break,
			}
		}
		trace!(target: "bitcoin-rs", "Block notification thread stopped");
	}
}

impl SyncListener for BlockNotifier {
	fn synchronization_state_switched(&self, is_synchronizing: bool) {
		self.is_synchronizing.store(is_synchronizing, Ordering::SeqCst);
	}

	fn best_storage_block_inserted(&self, block_hash: &SHA256D) {
		if !self.is_synchronizing.load(Ordering::SeqCst) {
			self.tx
				.send(BlockNotifierTask::NewBlock(*block_hash))
				.expect("Block notification thread have the same lifetime as `BlockNotifier`")
		}
	}
}

impl Drop for BlockNotifier {
	fn drop(&mut self) {
		if let Some(join_handle) = self.worker_thread.take() {
			let _ = self.tx.send(BlockNotifierTask::Stop);
			join_handle.join().expect("Clean shutdown.");
		}
	}
}

/// Setup functions in here spawn new threads (which should be done off the main thread).
pub fn start(runtime: &Runtime, db: SharedStore, cfg: config::Config) -> Result<(), String> {
	let sync_peers = create_sync_peers();
	let local_sync_node = create_local_sync_node(
		cfg.consensus.clone(),
		db.clone(),
		sync_peers.clone(),
		cfg.verification_params.clone(),
	);
	let sync_connection_factory = create_sync_connection_factory(sync_peers.clone(), local_sync_node.clone());

	if let Some(block_notify_command) = cfg.block_notify_command.clone() {
		local_sync_node.install_sync_listener(Box::new(BlockNotifier::new(block_notify_command)));
	}

	runtime.spawn(start_async(cfg, db, sync_connection_factory, local_sync_node));
	Ok(())
}

/// All work that happens in here is handled by the Tokio runtime.
pub async fn start_async(
	cfg: config::Config,
	db: SharedStore,
	sync_connection_factory: LocalSyncNodeRef,
	local_sync_node: LocalNodeRef,
) -> Result<(), String> {
	let p2p_cfg = p2p::Config {
		inbound_connections: cfg.inbound_connections,
		outbound_connections: cfg.outbound_connections,
		connection: p2p::NetConfig {
			protocol_version: PROTOCOL_VERSION,
			protocol_minimum: PROTOCOL_MINIMUM,
			network: cfg.consensus.network,
			local_address: SocketAddr::new(cfg.host.unwrap(), cfg.port),
			services: cfg.services,
			user_agent: cfg.user_agent,
			start_height: 0,
			relay: true,
		},
		peers: cfg.connect.map_or_else(|| vec![], |x| vec![x]),
		seed: cfg.seednode,
		node_table_path: node_table_path(&cfg.data_dir),
		preferable_services: cfg.services,
		internet_protocol: cfg.internet_protocol,
	};
	let p2p = p2p::P2P::new(p2p_cfg, sync_connection_factory).map_err(|e| e.to_string())?;

	let rpc_deps = rpc::Dependencies {
		db_path: db_path(&cfg.data_dir),
		network: cfg.network,
		storage: db,
		local_sync_node,
		p2p_context: p2p.context().clone(),
		memory: Arc::new(Memory::default()),
	};
	let _rpc_server = rpc::new_http(cfg.rpc_config, rpc_deps)?;

	p2p.run().await;

	Ok(())
}
