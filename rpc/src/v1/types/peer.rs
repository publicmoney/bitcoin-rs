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

#[derive(Serialize, Default)]
pub struct Peer {
	// Peer index
	id: usize,
	// The IP address and port of the peer
	addr: String,
	// Bind address of the connection to the peer
	addrbind: String,
	// Local address as reported by the peer
	addrlocal: String,
	// The services offered
	services: String,
	// Whether peer has asked us to relay transactions to it
	relaytxes: bool,
	// The time in seconds since epoch (Jan 1 1970 GMT) of the last send
	lastsend: u32,
	// The time in seconds since epoch (Jan 1 1970 GMT) of the last receive
	lastrecv: u32,
	// The total bytes sent
	bytessent: u32,
	// The total bytes received
	bytesrecv: u32,
	// The connection time in seconds since epoch (Jan 1 1970 GMT)
	conntime: u32,
	// The time offset in seconds
	timeoffset: u32,
	// ping time (if available)
	pingtime: u32,
	// minimum observed ping time (if any at all)
	minping: u32,
	// ping wait (if non-zero)
	pingwait: u32,
	// The peer version, such as 70001
	version: u32,
	// The string version
	subver: String,
	// Inbound (true) or Outbound (false)
	inbound: bool,
	// Whether connection was due to addnode/-connect or if it was an automatic/inbound connection
	addnode: bool,
	// The starting height (block) of the peer
	startingheight: u32,
	// The ban score
	banscore: u32,
	// The last header we have in common with this peer
	synced_headers: u32,
	// The last block we have in common with this peer
	synced_blocks: u32,
	// The heights of blocks we're currently asking from this peer
	inflight: Vec<u32>,
	// Whether the peer is whitelisted
	minfeefilter: u32,
	// The minimum fee rate for transactions this peer accepts
	whitelisted: bool,
	// Bytes sent per message. If message is not in list bytes sent is zero.
	bytessent_per_msg: Vec<BytesPerMessage>,
	// Bytes received per message. If message is not in list bytes sent is zero.
	bytesrecv_per_msg: Vec<BytesPerMessage>,
}

#[derive(Serialize, Default)]
pub struct BytesPerMessage {
	msg: String, // Message type
	n: u32,      // Number of bytes sent
}
