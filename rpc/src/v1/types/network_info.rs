#[derive(Serialize, Deserialize, Default)]
pub struct NetworkInfo {
	// Server version
	pub version: usize,
	// Server subversion string
	pub subversion: String,
	// Protocol version
	pub protocolversion: usize,
	// Services we offer to the network
	pub localservices: String,
	// Transaction relay is requested from peers
	pub localrelay: bool,
	// The time offset
	pub timeoffset: usize,
	// Number of connections
	pub connections: usize,
	// Whether p2p networking is enabled
	pub networkactive: bool,
	// information per network
	pub networks: Vec<Network>,
	// Minimum relay fee for transactions in BTC/kB
	pub relayfee: usize,
	// Minimum fee increment for mempool limiting or BIP 125 replacement in BTC/kB
	pub incrementalfee: usize,
	// List of local addresses
	pub localaddresses: Vec<Address>,
	// Any network and blockchain warnings
	pub warnings: String,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Address {
	// Network address
	pub address: String,
	// Network port
	pub port: usize,
	// Relative score
	pub score: usize,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Network {
	// Network (ipv4, ipv6 or onion)
	pub name: String,
	// Is the network limited using -onlynet?
	pub limited: bool,
	// Is the network reachable?
	pub reachable: bool,
	// The proxy that is used for this network, or empty if none
	pub proxy: String,
	// Whether randomized credentials are used
	pub proxy_randomize_credentials: bool,
}
