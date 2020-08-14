use crate::rpc::HttpConfiguration as RpcHttpConfig;
use crate::rpc_apis::ApiSet;
use crate::seednodes::{mainnet_seednodes, testnet_seednodes};
use crate::{REGTEST_USER_AGENT, USER_AGENT, USER_AGENT_VERSION};
use clap;
use message::Services;
use network::{ConsensusParams, Network};
use p2p::InternetProtocol;
use std::net;
use sync::VerificationParameters;
use verification::VerificationLevel;

pub struct Config {
	pub network: Network,
	pub consensus: ConsensusParams,
	pub services: Services,
	pub port: u16,
	pub connect: Option<net::SocketAddr>,
	pub host: net::IpAddr,
	pub seednodes: Vec<String>,
	pub quiet: bool,
	pub inbound_connections: u32,
	pub outbound_connections: u32,
	pub db_cache: usize,
	pub data_dir: Option<String>,
	pub user_agent: String,
	pub internet_protocol: InternetProtocol,
	pub rpc_config: RpcHttpConfig,
	pub block_notify_command: Option<String>,
	pub verification_params: VerificationParameters,
}

pub const DEFAULT_DB_CACHE: usize = 512;

pub fn parse(matches: &clap::ArgMatches) -> Result<Config, String> {
	let db_cache = match matches.value_of("db-cache") {
		Some(s) => s.parse().map_err(|_| "Invalid cache size - should be number in MB".to_owned())?,
		None => DEFAULT_DB_CACHE,
	};

	let data_dir = match matches.value_of("data-dir") {
		Some(s) => Some(s.parse().map_err(|_| "Invalid data-dir".to_owned())?),
		None => None,
	};

	let quiet = matches.is_present("quiet");
	let network = match (matches.is_present("testnet"), matches.is_present("regtest")) {
		(true, false) => Network::Testnet,
		(false, true) => Network::Regtest,
		(false, false) => Network::Mainnet,
		(true, true) => return Err("Only one testnet option can be used".into()),
	};

	let consensus = ConsensusParams::new(network);

	let (in_connections, out_connections) = match network {
		Network::Testnet | Network::Mainnet | Network::Other(_) => (10, 10),
		Network::Regtest | Network::Unitest => (1, 0),
	};

	let user_agent = match network {
		Network::Testnet | Network::Mainnet | Network::Unitest | Network::Other(_) => format!("{}:{}", USER_AGENT, USER_AGENT_VERSION),
		Network::Regtest => REGTEST_USER_AGENT.into(),
	};

	let port = match matches.value_of("port") {
		Some(port) => port.parse().map_err(|_| "Invalid port".to_owned())?,
		None => network.port(),
	};

	let connect = match matches.value_of("connect") {
		Some(s) => Some(match s.parse::<net::SocketAddr>() {
			Err(_) => s
				.parse::<net::IpAddr>()
				.map(|ip| net::SocketAddr::new(ip, network.port()))
				.map_err(|_| "Invalid connect".to_owned()),
			Ok(a) => Ok(a),
		}?),
		None => None,
	};

	let seednodes: Vec<String> = match matches.value_of("seednode") {
		Some(s) => vec![s.parse().map_err(|_| "Invalid seednode".to_owned())?],
		None => match network {
			Network::Mainnet => mainnet_seednodes().into_iter().map(Into::into).collect(),
			Network::Testnet => testnet_seednodes().into_iter().map(Into::into).collect(),
			Network::Other(_) | Network::Regtest | Network::Unitest => Vec::new(),
		},
	};

	let only_net = match matches.value_of("only-net") {
		Some(s) => s.parse()?,
		None => InternetProtocol::default(),
	};

	let host = match matches.value_of("host") {
		Some(s) => s.parse::<net::IpAddr>().map_err(|_| "Invalid host".to_owned())?,
		None => match only_net {
			InternetProtocol::IpV6 => "::".parse().unwrap(),
			_ => "0.0.0.0".parse().unwrap(),
		},
	};

	let rpc_config = parse_rpc_config(network, matches)?;

	let block_notify_command = match matches.value_of("blocknotify") {
		Some(s) => Some(s.parse().map_err(|_| "Invalid blocknotify commmand".to_owned())?),
		None => None,
	};

	let services = Services::default().with_network(true).with_witness(true);

	let verification_level = match matches.value_of("verification-level") {
		Some(s) if s == "full" => VerificationLevel::Full,
		Some(s) if s == "header" => VerificationLevel::Header,
		Some(s) if s == "none" => VerificationLevel::NoVerification,
		Some(s) => return Err(format!("Invalid verification level: {}", s)),
		None => VerificationLevel::Full,
	};

	let verification_edge = match matches.value_of("verification-edge") {
		Some(s) if verification_level != VerificationLevel::Full => s.parse().map_err(|_| "Invalid verification edge".to_owned())?,
		_ => network.default_verification_edge(),
	};

	let config = Config {
		quiet,
		network,
		consensus,
		services,
		port,
		connect,
		host,
		seednodes,
		inbound_connections: in_connections,
		outbound_connections: out_connections,
		db_cache,
		data_dir,
		user_agent,
		internet_protocol: only_net,
		rpc_config,
		block_notify_command,
		verification_params: VerificationParameters {
			verification_level,
			verification_edge,
		},
	};

	Ok(config)
}

fn parse_rpc_config(network: Network, matches: &clap::ArgMatches) -> Result<RpcHttpConfig, String> {
	let mut config = RpcHttpConfig::with_port(network.rpc_port());
	config.enabled = !matches.is_present("no-jsonrpc");
	if !config.enabled {
		return Ok(config);
	}

	if let Some(apis) = matches.value_of("jsonrpc-apis") {
		config.apis = ApiSet::List(vec![apis.parse().map_err(|_| "Invalid APIs".to_owned())?].into_iter().collect());
	}
	if let Some(port) = matches.value_of("jsonrpc-port") {
		config.port = port.parse().map_err(|_| "Invalid JSON RPC port".to_owned())?;
	}
	if let Some(interface) = matches.value_of("jsonrpc-interface") {
		config.interface = interface.to_owned();
	}
	if let Some(cors) = matches.value_of("jsonrpc-cors") {
		config.cors = Some(vec![cors.parse().map_err(|_| "Invalid JSON RPC CORS".to_owned())?]);
	}
	if let Some(hosts) = matches.value_of("jsonrpc-hosts") {
		config.hosts = Some(vec![hosts.parse().map_err(|_| "Invalid JSON RPC hosts".to_owned())?]);
	}

	Ok(config)
}
