use std::fmt::{Display, Formatter};

/// IO Top level error that might be caused by bad message format, IO or timeout.
/// Unable to implement PartialEq on this due to io::Error not implementing it.
#[derive(Debug)]
pub enum Error {
	Message(message::Error),
	IO(std::io::Error),
	Timeout,
}

impl From<std::io::Error> for Error {
	fn from(e: std::io::Error) -> Self {
		Error::IO(e)
	}
}

impl From<message::Error> for Error {
	fn from(e: message::Error) -> Self {
		Error::Message(e)
	}
}

impl From<tokio::time::error::Elapsed> for Error {
	fn from(_: tokio::time::error::Elapsed) -> Self {
		Error::Timeout
	}
}

impl std::error::Error for Error {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Error::Message(err) => Some(err),
			Error::IO(err) => Some(err),
			Error::Timeout => None,
		}
	}
}

impl Display for Error {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
		match self {
			Error::Message(err) => write!(f, "Message Error: {}", err),
			Error::IO(err) => write!(f, "IO Error: {}", err),
			Error::Timeout => write!(f, "Timeout Error"),
		}
	}
}
