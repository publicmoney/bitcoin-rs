#[cfg(test)]
use crate::bytes::Bytes;
use std::io;
#[cfg(test)]
use std::io::Cursor;
#[cfg(test)]
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;
#[cfg(not(test))]
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::Mutex;

#[cfg(not(test))]
pub struct SharedTcpStream {
	io: Arc<Mutex<TcpStream>>,
}

#[cfg(not(test))]
impl SharedTcpStream {
	pub async fn connect(address: &SocketAddr) -> Result<SharedTcpStream, io::Error> {
		let stream = TcpStream::connect(address).await?;
		Ok(Self::new(stream))
	}

	fn new(stream: TcpStream) -> Self {
		SharedTcpStream {
			io: Arc::new(Mutex::new(stream)),
		}
	}

	pub async fn shutdown(&self) {
		// error is irrelevant here, the connection is dropped anyway
		let _ = self.io.lock().await.shutdown(std::net::Shutdown::Both);
	}

	pub async fn write_all(&self, buf: &[u8]) -> Result<(), io::Error> {
		self.io.lock().await.write_all(buf).await
	}

	pub async fn read_exact(&self, buf: &mut [u8]) -> Result<usize, io::Error> {
		self.io.lock().await.read_exact(buf).await
	}
}

#[cfg(not(test))]
impl From<TcpStream> for SharedTcpStream {
	fn from(stream: TcpStream) -> Self {
		Self::new(stream)
	}
}

#[cfg(not(test))]
impl Clone for SharedTcpStream {
	fn clone(&self) -> Self {
		SharedTcpStream { io: self.io.clone() }
	}
}

/// Test Shared Stream
/// TODO when async functions are allowed in traits this can made generic instead of using cfg.
#[cfg(test)]
pub struct SharedTcpStream {
	read: Arc<Mutex<Cursor<Vec<u8>>>>,
	write: Arc<Mutex<Bytes>>,
}

#[cfg(test)]
impl SharedTcpStream {
	pub async fn connect(_address: &SocketAddr) -> Result<SharedTcpStream, io::Error> {
		Ok(Self::new(Bytes::new()))
	}

	pub fn new(bytes: Bytes) -> Self {
		SharedTcpStream {
			read: Arc::new(Mutex::new(Cursor::new(bytes.into()))),
			write: Arc::new(Mutex::new(Bytes::default())),
		}
	}

	pub async fn shutdown(&self) {}

	pub async fn get_written(&self) -> Bytes {
		let write = self.write.lock().await;
		write.clone()
	}

	pub async fn write_all(&self, buf: &[u8]) -> Result<(), io::Error> {
		let mut write = self.write.lock().await;
		write.write(buf).await?;
		Ok(())
	}

	pub async fn read_exact(&self, buf: &mut [u8]) -> Result<usize, io::Error> {
		let mut read = self.read.lock().await;
		println!("{:?}", read.get_ref());
		if buf.len() > read.get_ref().len() {
			return Err(io::Error::from(ErrorKind::UnexpectedEof));
		}
		std::io::Read::read(&mut *read, buf)?;

		println!("buf {:?}", buf);

		Ok(buf.len())
	}
}

#[cfg(test)]
impl From<TcpStream> for SharedTcpStream {
	fn from(_stream: TcpStream) -> Self {
		Self::new(Bytes::default())
	}
}
