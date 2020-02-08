use crate::bytes::Bytes;
use crate::io::{read_any_message, SharedTcpStream, Error};
use crate::session::Session;
use crate::PeerInfo;
use message::Command;

pub struct Channel {
	stream: SharedTcpStream,
	peer_info: PeerInfo,
	session: Session,
}

impl Channel {
	pub fn new(stream: SharedTcpStream, peer_info: PeerInfo, session: Session) -> Self {
		Channel {
			stream,
			peer_info,
			session,
		}
	}

	pub async fn write_message<T>(&self, message: T) -> std::io::Result<()>
	where
		T: AsRef<[u8]>,
	{
		self.stream.write_all(message.as_ref()).await
	}

	pub async fn read_message(&self) -> Result<(Command, Bytes), Error> {
		read_any_message(&self.stream, self.peer_info.magic).await
	}

	pub async fn shutdown(&self) {
		self.stream.shutdown().await;
	}

	pub fn version(&self) -> u32 {
		self.peer_info.version
	}

	pub fn peer_info(&self) -> PeerInfo {
		self.peer_info.clone()
	}

	pub fn session(&self) -> &Session {
		&self.session
	}
}
