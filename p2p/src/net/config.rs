use crate::util::nonce::{NonceGenerator, RandomNonce};
use message::common::{NetAddress, Services};
use message::types::version::{Version, V0, V106, V70001};
use network::Network;
use primitives::time::{RealTime, Time};
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct Config {
	pub protocol_version: u32,
	pub protocol_minimum: u32,
	pub network: Network,
	pub local_address: SocketAddr,
	pub services: Services,
	pub user_agent: String,
	pub start_height: u32,
	pub relay: bool,
}

impl Config {
	pub fn version(&self, to: &SocketAddr) -> Version {
		Version::V70001(
			V0 {
				version: self.protocol_version,
				services: self.services,
				timestamp: RealTime.now().as_secs(),
				receiver: NetAddress {
					services: self.services,
					address: to.ip().into(),
					port: to.port().into(),
				},
			},
			V106 {
				from: NetAddress {
					services: self.services,
					address: self.local_address.ip().into(),
					port: self.local_address.port().into(),
				},
				nonce: RandomNonce.get(),
				user_agent: self.user_agent.clone(),
				start_height: self.start_height,
			},
			V70001 { relay: self.relay },
		)
	}
}
