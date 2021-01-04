use crate::rpc_apis::ApiSet;
use crate::{rpc_apis, Compatibility, MetaIoHandler, Server};
use jsonrpc_http_server::{Host, ServerBuilder};
use memory::Memory;
use network::Network;
use p2p;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use storage;
use sync;
use tokio::sync::Notify;

pub struct Dependencies {
	pub network: Network,
	pub local_sync_node: sync::LocalNodeRef,
	pub storage: storage::SharedStore,
	pub p2p_context: Arc<p2p::Context>,
	pub memory: Arc<Memory>,
	pub shutdown_signal: Arc<Notify>,
}

#[derive(Default, Debug, PartialEq)]
pub struct HttpConfiguration {
	pub enabled: bool,
	pub interface: String,
	pub port: u16,
	pub apis: ApiSet,
	pub cors: Option<Vec<String>>,
	pub hosts: Option<Vec<String>>,
}

impl HttpConfiguration {
	pub fn with_port(port: u16) -> Self {
		HttpConfiguration {
			enabled: true,
			interface: "127.0.0.1".into(),
			port,
			apis: ApiSet::default(),
			cors: None,
			hosts: Some(Vec::new()),
		}
	}
}

pub fn new_http(conf: HttpConfiguration, deps: Dependencies) -> Result<Option<Server>, String> {
	if !conf.enabled {
		return Ok(None);
	}

	let url = format!("{}:{}", conf.interface, conf.port);
	let addr = url
		.parse()
		.map_err(|_| format!("Invalid JSONRPC listen host/port given: {}", url))?;
	Ok(Some(setup_http_rpc_server(&addr, conf.cors, conf.hosts, conf.apis, deps)?))
}

pub fn setup_http_rpc_server(
	url: &SocketAddr,
	cors_domains: Option<Vec<String>>,
	allowed_hosts: Option<Vec<String>>,
	apis: ApiSet,
	deps: Dependencies,
) -> Result<Server, String> {
	let server = setup_rpc_server(apis, deps);
	let start_result = start_http(url, cors_domains, allowed_hosts, server);
	match start_result {
		Err(ref err) if err.kind() == io::ErrorKind::AddrInUse => {
			Err(format!("RPC address {} is already in use, make sure that another instance of a Bitcoin node is not running or change the address using the --jsonrpc-port and --jsonrpc-interface options.", url))
		},
		Err(e) => Err(format!("RPC error: {:?}", e)),
		Ok(server) => Ok(server),
	}
}

fn setup_rpc_server(apis: ApiSet, deps: Dependencies) -> MetaIoHandler<()> {
	rpc_apis::setup_rpc(MetaIoHandler::with_compatibility(Compatibility::Both), apis, deps)
}

fn start_http<M: std::default::Default + jsonrpc_core::Metadata>(
	addr: &SocketAddr,
	cors_domains: Option<Vec<String>>,
	allowed_hosts: Option<Vec<String>>,
	handler: jsonrpc_core::MetaIoHandler<M>,
) -> Result<Server, io::Error> {
	let cors_domains = cors_domains.map(|domains| {
		domains
			.into_iter()
			.map(|v| match v.as_str() {
				"*" => jsonrpc_http_server::AccessControlAllowOrigin::Any,
				"null" => jsonrpc_http_server::AccessControlAllowOrigin::Null,
				v => jsonrpc_http_server::AccessControlAllowOrigin::Value(v.into()),
			})
			.collect()
	});

	ServerBuilder::new(handler)
		.cors(cors_domains.into())
		//		.event_loop_executor(executor) TODO use existing tokio runtime instead of starting a new one (when jsonrpc-http-server has upgraded to tokio 0.3)
		.allowed_hosts(allowed_hosts.map(|hosts| hosts.into_iter().map(Host::from).collect()).into())
		.start_http(addr)
}
