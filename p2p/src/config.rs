use crate::{InternetProtocol, NetConfig};
use message::common::Services;
use std::net;
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct Config {
	/// Number of inbound connections.
	pub inbound_connections: u32,
	/// Number of outbound connections.
	pub outbound_connections: u32,
	/// Configuration for every connection.
	pub connection: NetConfig,
	/// Connect only to these nodes.
	pub peers: Vec<net::SocketAddr>,
	/// Optional seed node to use instead of default bitcoin-core seeds.
	pub seed: Option<SocketAddr>,
	/// p2p/nodes.csv file path.
	pub node_table_path: String,
	/// Peers with this services will get a boost in node_table.
	pub preferable_services: Services,
	/// Internet protocol.
	pub internet_protocol: InternetProtocol,
}
