use ser::Error as ReaderError;
use std::{error, fmt};

pub type MessageResult<T> = Result<T, Error>;

#[derive(Debug, PartialEq, Clone)]
pub enum Error {
	/// Deserialization failed.
	Deserialize,
	/// Command has wrong format or is unsupported.
	InvalidCommand,
	/// Network magic comes from different network.
	InvalidMagic,
	/// Invalid checksum.
	InvalidChecksum,
	/// Invalid version.
	InvalidVersion,
}

impl From<ReaderError> for Error {
	fn from(_: ReaderError) -> Self {
		Error::Deserialize
	}
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let message = match *self {
			Error::Deserialize => "Message Deserialization Error",
			Error::InvalidCommand => "Invalid Message Command",
			Error::InvalidMagic => "Invalid Network Magic",
			Error::InvalidChecksum => "Invalid message checksum",
			Error::InvalidVersion => "Unsupported protocol version",
		};
		f.write_str(message)
	}
}

impl error::Error for Error {}
