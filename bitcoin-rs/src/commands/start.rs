use crate::app_dir::app_path;
use crate::block_notifier::BlockNotifier;
use crate::config;
use memory::Memory;
use network::network::{PROTOCOL_MINIMUM, PROTOCOL_VERSION};
use p2p::P2P;
use rpc::Server;
use std::net::SocketAddr;
use std::sync::Arc;
use storage::SharedStore;
use sync::{create_local_sync_node, create_sync_connection_factory, create_sync_peers, LocalNodeRef};
use tokio::runtime::Runtime;

/// Some setup functions in here spawn new threads (which should be done off the main thread)
/// At the moment only the p2p context runs on the Tokio runtime. RPC servier has its own Tokio runtime.
pub fn start(runtime: &Runtime, db: SharedStore, cfg: config::Config) -> Result<(LocalNodeRef, P2P, Server), String> {
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
		node_table_path: app_path(&cfg.data_dir, "p2p"),
		preferable_services: cfg.services,
		internet_protocol: cfg.internet_protocol,
	};
	let p2p_context = Arc::new(p2p::Context::new(runtime.handle().clone(), sync_connection_factory, p2p_cfg).map_err(|e| e.to_string())?);
	let p2p = p2p::P2P::new(p2p_context.clone());

	let rpc_deps = rpc::rpc_server::Dependencies {
		network: cfg.network,
		storage: db,
		local_sync_node: local_sync_node.clone(),
		p2p_context,
		memory: Arc::new(Memory::default()),
	};
	let rpc_server = rpc::rpc_server::new_http(cfg.rpc_config, rpc_deps)?.unwrap();

	let p2p2 = p2p.clone();
	runtime.spawn(async move { p2p2.run().await });

	Ok((local_sync_node, p2p, rpc_server))
}
