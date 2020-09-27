use crate::io::Error;
use crate::net::{accept_connection, connect, Channel, ConnectionCounter, Connections};
use crate::session::{NormalSessionFactory, SeednodeSessionFactory, SessionFactory};
use crate::util::{Node, NodeTable};
use crate::{Config, Direction, InboundSyncConnectionRef, LocalSyncNodeRef, NetConfig, NodeTableError, OutboundSyncConnectionRef, PeerId};
use message::common::Services;
use message::types::addr::AddressEntry;
use message::{Message, Payload};
use network::Network;
use parking_lot::RwLock;
use rand::seq::SliceRandom;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::{error, net, time};
use tokio::runtime::Handle;
use tokio::time::delay_for;
use tokio::time::interval;
use tokio::{net::TcpListener, net::TcpStream, stream::StreamExt};

/// Network context.
pub struct Context {
	runtime_handle: Handle,
	/// Connections.
	connections: Connections,
	/// Connection counter.
	connection_counter: ConnectionCounter,
	/// Node Table.
	node_table: RwLock<NodeTable>,
	/// Local synchronization node.
	local_sync_node: LocalSyncNodeRef,
	/// Node table path.
	config: Config,
}

impl Context {
	/// Creates new context with reference to local sync node.
	pub fn new(local_sync_node: LocalSyncNodeRef, config: Config) -> Result<Self, Box<dyn error::Error>> {
		let context = Context {
			runtime_handle: tokio::runtime::Handle::current(),
			connections: Default::default(),
			connection_counter: ConnectionCounter::new(config.inbound_connections, config.outbound_connections),
			node_table: RwLock::new(NodeTable::from_file(config.preferable_services, config.node_table_path.clone())?),
			local_sync_node,
			config,
		};

		Ok(context)
	}

	pub fn get_user_agent(&self) -> String {
		self.config.connection.user_agent.clone()
	}

	pub fn get_version(&self) -> u32 {
		self.config.connection.protocol_version
	}

	/// Spawns a future using thread pool and schedules execution of it with event loop handle.
	pub fn spawn<F>(&self, f: F)
	where
		F: Future + Send + 'static,
		F::Output: Send,
	{
		self.runtime_handle.spawn(f);
	}

	/// Schedules execution of function in future.
	/// Use wisely, it keeps used objects in memory until after it is resolved.
	pub fn execute_after<F>(&self, duration: time::Duration, f: F)
	where
		F: FnOnce() + 'static + Send,
	{
		self.spawn(async move {
			delay_for(duration).await;
			f();
		});
	}

	/// Returns addresses of recently active nodes. Sorted and limited to 1000.
	pub fn node_table_entries(&self) -> Vec<Node> {
		self.node_table.read().recently_active_nodes(self.config.internet_protocol)
	}

	/// Updates node table.
	pub fn update_node_table(&self, nodes: Vec<AddressEntry>) {
		trace!("Updating node table with {} entries", nodes.len());
		self.node_table.write().insert_many(nodes);
	}

	/// Penalize node.
	pub fn penalize_node(&self, addr: &SocketAddr) {
		trace!("Penalizing node {}", addr);
		self.node_table.write().note_failure(addr);
	}

	/// Adds node to table.
	pub fn add_node(&self, addr: SocketAddr) -> Result<(), NodeTableError> {
		trace!("Adding node {} to node table", &addr);
		self.node_table.write().add(addr, self.config.connection.services)
	}

	/// Removes node from table.
	pub fn remove_node(&self, addr: SocketAddr) -> Result<(), NodeTableError> {
		trace!("Removing node {} from node table", &addr);
		self.node_table.write().remove(&addr)
	}

	/// Every 10 seconds check if we have reached maximum number of outbound connections.
	/// If not, connect to best peers.
	pub fn autoconnect(context: Arc<Context>) {
		let mut interval = interval(time::Duration::new(60, 0));
		let inner_context = context.clone();
		let interval_loop = async move {
			loop {
				interval.tick().await;
				tokio::spawn(Self::autoconnect_future(inner_context.clone()));
			}
		};
		tokio::spawn(interval_loop);
	}

	async fn autoconnect_future(context: Arc<Context>) {
		let ic = context.connection_counter.inbound_connections();
		let oc = context.connection_counter.outbound_connections();
		info!("Inbound connections: ({}/{})", ic.0, ic.1);
		info!("Outbound connections: ({}/{})", oc.0, oc.1);

		for channel in context.connections.channels().values() {
			channel.session().maintain();
		}

		let needed = context.connection_counter.outbound_connections_needed() as usize;
		if needed != 0 {
			let used_addresses = context.connections.addresses();
			let peers = context.node_table.read().nodes_with_services(
				&Services::default(),
				context.config.internet_protocol,
				&used_addresses,
				needed,
			);
			let addresses = peers.into_iter().map(|peer| peer.address()).collect::<Vec<_>>();

			trace!("Creating {} more outbound connections", addresses.len());
			for address in addresses {
				Context::connect::<NormalSessionFactory>(context.clone(), address);
			}
		}

		if let Err(_err) = context.node_table.read().save_to_file() {
			error!("Saving node table to disk failed");
		}
	}

	/// Connect to socket.
	async fn connect_future<T>(context: Arc<Context>, socket: net::SocketAddr, config: NetConfig)
	where
		T: SessionFactory,
	{
		trace!("Trying to connect to: {}", socket);
		match connect(&socket, &config).await {
			Ok(connection) => {
				// successful handshake
				trace!("Connected to {}", connection.address);
				context.node_table.write().insert(connection.address, connection.services);
				let channel = context.connections.store::<T>(context.clone(), connection, Direction::Outbound);

				// initialize session and then start reading messages
				channel.session().initialize();
				loop {
					if Context::on_message(context.clone(), channel.clone()).await.is_err() {
						break;
					}
				}
			}
			Err(Error::Message(err)) => {
				// protocol error
				trace!("Handshake with {} failed with {}", socket, err);
				// TODO: close socket
				context.node_table.write().note_failure(&socket);
				context.connection_counter.note_close_outbound_connection();
			}
			Err(Error::Timeout) => {
				// connection time out
				trace!("Handshake with {} timed out", socket);
				// TODO: close socket
				context.node_table.write().note_failure(&socket);
				context.connection_counter.note_close_outbound_connection();
			}
			Err(Error::IO(err)) => {
				// network error
				trace!("Failed to connect to {} with {}", socket, err);
				context.node_table.write().note_failure(&socket);
				context.connection_counter.note_close_outbound_connection();
			}
		}
	}

	/// Connect to socket using given context.
	pub fn connect<T>(context: Arc<Context>, socket: net::SocketAddr)
	where
		T: SessionFactory + 'static,
	{
		context.connection_counter.note_new_outbound_connection();
		let config = context.config.clone();
		tokio::spawn(Context::connect_future::<T>(context.clone(), socket, config.connection.clone()));
	}

	pub fn connect_normal(context: Arc<Context>, socket: net::SocketAddr) {
		Self::connect::<NormalSessionFactory>(context, socket)
	}

	pub async fn accept_connection_future(context: Arc<Context>, stream: TcpStream, socket: net::SocketAddr, config: NetConfig) {
		match accept_connection(stream, &config, socket).await {
			Ok(connection) => {
				// successful handshake
				trace!("Accepted connection from {}", connection.address);
				context.node_table.write().insert(connection.address, connection.services);
				let channel = context
					.connections
					.store::<NormalSessionFactory>(context.clone(), connection, Direction::Inbound);

				// initialize session and then start reading messages
				channel.session().initialize();
				let _ = Context::on_message(context.clone(), channel).await;
			}
			Err(Error::Message(err)) => {
				// protocol error
				trace!("Accepting handshake from {} failed with error: {}", socket, err);
				// TODO: close socket
				context.node_table.write().note_failure(&socket);
				context.connection_counter.note_close_inbound_connection();
			}
			Err(Error::Timeout) => {
				// connection time out
				trace!("Accepting handshake from {} timed out", socket);
				// TODO: close socket
				context.node_table.write().note_failure(&socket);
				context.connection_counter.note_close_inbound_connection();
			}
			_ => {
				// network error
				trace!("Accepting handshake from {} failed with network error", socket);
				context.node_table.write().note_failure(&socket);
				context.connection_counter.note_close_inbound_connection();
			}
		}
	}

	pub fn accept_connection(context: Arc<Context>, stream: TcpStream, socket: net::SocketAddr, config: NetConfig) {
		context.connection_counter.note_new_inbound_connection();
		tokio::spawn(Context::accept_connection_future(context.clone(), stream, socket, config));
	}

	/// Starts tcp server and listens for incoming connections.
	pub async fn listen(context: Arc<Context>, config: NetConfig) {
		trace!("Starting tcp server");
		let mut server = TcpListener::bind(&config.local_address).await.expect("Unable to bind to address");
		let mut incoming = server.incoming();
		while let Some(stream) = incoming.next().await {
			match stream {
				Ok(stream) => {
					// because we acquire atomic value twice,
					// it may happen that accept slightly more connections than we need
					// we don't mind
					if context.connection_counter.inbound_connections_needed() > 0 {
						Context::accept_connection(context.clone(), stream, config.local_address, config.clone());
					} else {
						// ignore result
						let _ = stream.shutdown(net::Shutdown::Both);
					}
				}
				Err(_) => { /* connection failed */ }
			}
		}
	}

	/// Called on incoming message.
	pub async fn on_message(context: Arc<Context>, channel: Arc<Channel>) -> Result<(), Error> {
		let result = channel.read_message().await;
		match result {
			Ok((command, payload)) => {
				// successful read
				trace!("Received {} message from {}", command, channel.peer_info().address);
				// handle message and read the next one
				match channel.session().on_message(command, payload) {
					Ok(_) => {
						context.node_table.write().note_used(&channel.peer_info().address);
						Ok(())
					}
					Err(err) => {
						// protocol error
						context.close_channel_with_error(channel.peer_info().id, &err);
						Err(err)
					}
				}
			}
			Err(err) => {
				// network error
				// TODO: remote node was just turned off. should we mark it as not reliable?
				context.close_channel_with_error(channel.peer_info().id, &err);
				Err(err)
			}
		}
	}

	/// Send message to a channel with given peer id.
	pub async fn send_to_peer<T>(context: Arc<Context>, peer: PeerId, payload: T, serialization_flags: u32)
	where
		T: Payload,
	{
		match context.connections.channel(peer) {
			Some(channel) => {
				let info = channel.peer_info();
				let message = Message::with_flags(info.magic, info.version, &payload, serialization_flags)
					.expect("failed to create outgoing message");
				channel.session().stats().lock().report_send(T::command().into(), message.len());
				Context::send(context, channel, message).await
			}
			None => {
				// peer no longer exists.
				// TODO: should we return error here?
			}
		}
	}

	pub async fn send_message_to_peer<T>(context: Arc<Context>, peer: PeerId, message: T)
	where
		T: AsRef<[u8]> + Send + 'static,
	{
		match context.connections.channel(peer) {
			Some(channel) => Context::send(context, channel, message).await,
			None => {
				// peer no longer exists.
				// TODO: should we return error here?
			}
		}
	}

	/// Send message using given channel.
	pub async fn send<T>(context: Arc<Context>, channel: Arc<Channel>, message: T)
	where
		T: AsRef<[u8]> + Send + 'static,
	{
		//		trace!("Sending {} message to {}", T::command(), channel.peer_info().address);
		match channel.write_message(message).await {
			Ok(_) => {
				// successful send
				//				trace!("Sent {} message to {}", T::command(), channel.peer_info().address);
			}
			Err(err) => {
				// network error
				context.close_channel_with_error(channel.peer_info().id, &err);
			}
		}
	}

	/// Close channel with given peer info.
	pub fn close_channel(&self, id: PeerId) {
		if let Some(channel) = self.connections.remove(id) {
			let info = channel.peer_info();
			channel.session().on_close();
			trace!("Disconnecting from {}", info.address);
			tokio::spawn(async move { channel.shutdown().await });
			match info.direction {
				Direction::Inbound => self.connection_counter.note_close_inbound_connection(),
				Direction::Outbound => self.connection_counter.note_close_outbound_connection(),
			}
		}
	}

	/// Close channel with given peer info.
	pub fn close_channel_with_error(&self, id: PeerId, error: &dyn error::Error) {
		if let Some(channel) = self.connections.remove(id) {
			let info = channel.peer_info();
			channel.session().on_close();
			trace!("Disconnecting from {} caused by {}", info.address, error);
			tokio::spawn(async move { channel.shutdown().await });
			self.node_table.write().note_failure(&info.address);
			match info.direction {
				Direction::Inbound => self.connection_counter.note_close_inbound_connection(),
				Direction::Outbound => self.connection_counter.note_close_outbound_connection(),
			}
		}
	}

	pub fn create_sync_session(
		&self,
		start_height: i32,
		services: Services,
		outbound_connection: OutboundSyncConnectionRef,
	) -> InboundSyncConnectionRef {
		self.local_sync_node
			.create_sync_session(start_height, services, outbound_connection)
	}

	pub fn connections(&self) -> &Connections {
		&self.connections
	}

	pub fn nodes(&self) -> Vec<Node> {
		self.node_table.read().nodes()
	}
}

include!(concat!(env!("OUT_DIR"), "/seeds_main.rs"));
include!(concat!(env!("OUT_DIR"), "/seeds_test.rs"));

pub struct P2P {
	/// P2P config.
	pub config: Config,
	/// Network context.
	context: Arc<Context>,
}

impl Drop for P2P {
	fn drop(&mut self) {
		// there are retain cycles
		// context->connections->channel->session->protocol->context
		// context->connections->channel->on_message closure->context
		// first let's get rid of session retain cycle
		for channel in self.context.connections.remove_all() {
			// done, now let's finish on_message
			tokio::spawn(async move { channel.shutdown().await });
		}
	}
}

impl P2P {
	pub fn new(config: Config, local_sync_node: LocalSyncNodeRef) -> Result<Self, Box<dyn error::Error>> {
		let context = Context::new(local_sync_node, config.clone())?;

		Ok(P2P {
			context: Arc::new(context),
			config,
		})
	}

	pub async fn run(self) {
		for peer in &self.config.peers {
			self.connect::<NormalSessionFactory>(*peer);
		}

		if self.config.seed.is_some() {
			Context::connect::<SeednodeSessionFactory>(self.context.clone(), self.config.seed.unwrap());
		} else {
			let seeds: Vec<SocketAddr> = match self.config.connection.network {
				Network::Mainnet => seeds_main(),
				Network::Testnet => seeds_test(),
				_ => vec![],
			};

			for seed in seeds.choose_multiple(&mut rand::thread_rng(), 5) {
				Context::connect::<SeednodeSessionFactory>(self.context.clone(), *seed);
			}
		}

		Context::autoconnect(self.context.clone());

		Context::listen(self.context.clone(), self.config.connection.clone()).await
	}

	/// Attempts to connect to the specified node
	pub fn connect<T>(&self, addr: net::SocketAddr)
	where
		T: SessionFactory + 'static,
	{
		Context::connect::<T>(self.context.clone(), addr);
	}

	pub fn context(&self) -> &Arc<Context> {
		&self.context
	}
}
