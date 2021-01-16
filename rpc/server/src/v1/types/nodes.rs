use p2p::{Direction, PeerInfo};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum AddNodeOperation {
	#[serde(rename = "add")]
	Add,
	#[serde(rename = "remove")]
	Remove,
	#[serde(rename = "onetry")]
	OneTry,
}

#[derive(Serialize, Deserialize)]
pub struct NodeInfoAddress {
	address: String,
	connected: NodeInfoAddressConnectionType,
}

impl From<PeerInfo> for NodeInfoAddress {
	fn from(info: PeerInfo) -> Self {
		NodeInfoAddress {
			address: format!("{}", info.address),
			connected: match info.direction {
				Direction::Inbound => NodeInfoAddressConnectionType::Inbound,
				Direction::Outbound => NodeInfoAddressConnectionType::Outbound,
			},
		}
	}
}

#[derive(Serialize, Deserialize)]
pub struct NodeInfo {
	pub addednode: String,
	pub connected: bool,
	pub addresses: Vec<NodeInfoAddress>,
}

#[derive(Serialize, Deserialize)]
pub enum NodeInfoAddressConnectionType {
	#[serde(rename = "inbound")]
	Inbound,
	#[serde(rename = "outbound")]
	Outbound,
}
