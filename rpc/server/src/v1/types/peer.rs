use p2p::{Direction, PeerInfo};

impl From<&PeerInfo> for Peer {
	fn from(info: &PeerInfo) -> Self {
		Peer {
			id: info.id,
			addr: info.address.to_string(),
			inbound: match info.direction {
				Direction::Inbound => true,
				Direction::Outbound => false,
			},
			subver: info.user_agent.clone(),
			version: info.version,
			..Default::default()
		}
	}
}

#[derive(Serialize, Deserialize, Default)]
pub struct Peer {
	// Peer index
	pub id: usize,
	// The IP address and port of the peer
	pub addr: String,
	// Bind address of the connection to the peer
	pub addrbind: String,
	// Local address as reported by the peer
	pub addrlocal: String,
	// The services offered
	pub services: String,
	// Whether peer has asked us to relay transactions to it
	pub relaytxes: bool,
	// The time in seconds since epoch (Jan 1 1970 GMT) of the last send
	pub lastsend: u32,
	// The time in seconds since epoch (Jan 1 1970 GMT) of the last receive
	pub lastrecv: u32,
	// The total bytes sent
	pub bytessent: u32,
	// The total bytes received
	pub bytesrecv: u32,
	// The connection time in seconds since epoch (Jan 1 1970 GMT)
	pub conntime: u32,
	// The time offset in seconds
	pub timeoffset: u32,
	// ping time (if available)
	pub pingtime: u32,
	// minimum observed ping time (if any at all)
	pub minping: u32,
	// ping wait (if non-zero)
	pub pingwait: u32,
	// The peer version, such as 70001
	pub version: u32,
	// The string version
	pub subver: String,
	// Inbound (true) or Outbound (false)
	pub inbound: bool,
	// Whether connection was due to addnode/-connect or if it was an automatic/inbound connection
	pub addnode: bool,
	// The starting height (block) of the peer
	pub startingheight: u32,
	// The ban score
	pub banscore: u32,
	// The last header we have in common with this peer
	pub synced_headers: u32,
	// The last block we have in common with this peer
	pub synced_blocks: u32,
	// The heights of blocks we're currently asking from this peer
	pub inflight: Vec<u32>,
	// Whether the peer is whitelisted
	pub minfeefilter: u32,
	// The minimum fee rate for transactions this peer accepts
	pub whitelisted: bool,
	// Bytes sent per message. If message is not in list bytes sent is zero.
	pub bytessent_per_msg: Vec<BytesPerMessage>,
	// Bytes received per message. If message is not in list bytes sent is zero.
	pub bytesrecv_per_msg: Vec<BytesPerMessage>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct BytesPerMessage {
	pub msg: String, // Message type
	pub n: u32,      // Number of bytes sent
}
