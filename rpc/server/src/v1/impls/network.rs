use crate::v1::helpers::errors;
use crate::v1::traits::Network as NetworkRpc;
use crate::v1::types::{AddNodeOperation, NodeInfo};
use crate::v1::types::{NetworkInfo, Peer};
use jsonrpc_core::Error;
use p2p;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

pub trait NetworkApi: Send + Sync + 'static {
	fn add_node(&self, socket_addr: SocketAddr) -> Result<(), p2p::NodeTableError>;
	fn remove_node(&self, socket_addr: SocketAddr) -> Result<(), p2p::NodeTableError>;
	fn connect(&self, socket_addr: SocketAddr);
	fn node_info(&self, node_addr: IpAddr) -> Result<NodeInfo, p2p::NodeTableError>;
	fn nodes_info(&self) -> Vec<NodeInfo>;
	fn peers_info(&self) -> Vec<Peer>;
	fn connection_count(&self) -> usize;
	fn network_info(&self) -> NetworkInfo;
}

impl<T> NetworkRpc for NetworkClient<T>
where
	T: NetworkApi,
{
	fn network_info(&self) -> Result<NetworkInfo, Error> {
		Ok(self.api.network_info())
	}

	fn add_node(&self, node: String, operation: AddNodeOperation) -> Result<(), Error> {
		let addr = node
			.parse()
			.map_err(|_| errors::invalid_params("node", "Invalid socket address format, should be ip:port (127.0.0.1:8008)"))?;
		match operation {
			AddNodeOperation::Add => self.api.add_node(addr).map_err(|_| errors::node_already_added()),
			AddNodeOperation::Remove => self.api.remove_node(addr).map_err(|_| errors::node_not_added()),
			AddNodeOperation::OneTry => {
				self.api.connect(addr);
				Ok(())
			}
		}
	}

	fn node_info(&self, _dns: bool, node_addr: Option<String>) -> Result<Vec<NodeInfo>, Error> {
		Ok(match node_addr {
			None => self.api.nodes_info(),
			Some(node_addr) => {
				let addr = node_addr
					.parse()
					.map_err(|_| errors::invalid_params("node", "Invalid ip address format, should be ip address (127.0.0.1)"))?;
				let node_info = self.api.node_info(addr).map_err(|_| errors::node_not_added())?;
				vec![node_info]
			}
		})
	}

	fn connection_count(&self) -> Result<usize, Error> {
		Ok(self.api.connection_count())
	}

	fn peer_info(&self) -> Result<Vec<Peer>, Error> {
		Ok(self.api.peers_info())
	}
}

pub struct NetworkClient<T: NetworkApi> {
	api: T,
}

impl<T> NetworkClient<T>
where
	T: NetworkApi,
{
	pub fn new(api: T) -> Self {
		NetworkClient { api }
	}
}

pub struct NetworkClientCore {
	p2p: Arc<p2p::Context>,
}

impl NetworkClientCore {
	pub fn new(p2p: Arc<p2p::Context>) -> Self {
		NetworkClientCore { p2p }
	}
}

impl NetworkApi for NetworkClientCore {
	fn add_node(&self, socket_addr: SocketAddr) -> Result<(), p2p::NodeTableError> {
		self.p2p.add_node(socket_addr)
	}

	fn remove_node(&self, socket_addr: SocketAddr) -> Result<(), p2p::NodeTableError> {
		self.p2p.remove_node(socket_addr)
	}

	fn connect(&self, socket_addr: SocketAddr) {
		p2p::Context::connect_normal(self.p2p.clone(), socket_addr);
	}

	fn node_info(&self, node_addr: IpAddr) -> Result<NodeInfo, p2p::NodeTableError> {
		let exact_node = self
			.p2p
			.nodes()
			.iter()
			.find(|n| n.address().ip() == node_addr)
			.cloned()
			.ok_or(p2p::NodeTableError::NoAddressInTable)?;

		let peers: Vec<p2p::PeerInfo> = self
			.p2p
			.connections()
			.info()
			.into_iter()
			.filter(|p| p.address == exact_node.address())
			.collect();

		Ok(NodeInfo {
			addednode: format!("{}", exact_node.address()),
			connected: !peers.is_empty(),
			addresses: peers.into_iter().map(|p| p.into()).collect(),
		})
	}

	fn nodes_info(&self) -> Vec<NodeInfo> {
		let peers: Vec<p2p::PeerInfo> = self.p2p.connections().info();

		self.p2p
			.nodes()
			.iter()
			.map(|n| {
				let node_peers: Vec<p2p::PeerInfo> = peers.iter().filter(|p| p.address == n.address()).cloned().collect();
				NodeInfo {
					addednode: format!("{}", n.address()),
					connected: !node_peers.is_empty(),
					addresses: node_peers.into_iter().map(|p| p.into()).collect(),
				}
			})
			.collect()
	}

	fn peers_info(&self) -> Vec<Peer> {
		self.p2p.connections().info().iter().map(|p| p.into()).collect()
	}

	fn connection_count(&self) -> usize {
		self.p2p.connections().count()
	}

	fn network_info(&self) -> NetworkInfo {
		NetworkInfo {
			connections: self.p2p.connections().count(),
			incrementalfee: 0,
			localaddresses: Vec::new(),
			localrelay: false,
			localservices: "".to_string(),
			networkactive: true,
			networks: Vec::new(),
			protocolversion: self.p2p.get_version() as usize,
			relayfee: 0,
			version: 0,
			subversion: self.p2p.get_user_agent(),
			timeoffset: 0,
			warnings: "".to_string(),
		}
	}
}
