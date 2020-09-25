use std::convert;
use std::fmt;
use std::io;
use std::sync;

/// Errors returned by this library
pub enum Error {
	/// pref is invalid (> 2^48)
	InvalidOffset,
	/// corrupted data
	Corrupted(String),
	/// key too long
	KeyTooLong,
	/// wrapped IO error
	IO(io::Error),
	/// Lock poisoned
	Poisoned(String),
	/// Queue error
	Queue(String),
	/// Value does not fit in given space
	ValueTooLong,
}

impl Error {
	fn description(&self) -> String {
		match *self {
			Error::InvalidOffset => "Invalid PRef".to_string(),
			Error::KeyTooLong => "Key too long".to_string(),
			Error::Corrupted(ref s) => format!("Corrupted: {}", s),
			Error::IO(_) => "IO Error".to_string(),
			Error::Poisoned(ref s) => format!("Poisoned: {}", s),
			Error::Queue(ref s) => format!("Queue: {}", s),
			Error::ValueTooLong => "Value too long".to_string(),
		}
	}
}

impl std::error::Error for Error {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match *self {
			Error::InvalidOffset => None,
			Error::KeyTooLong => None,
			Error::Corrupted(_) => None,
			Error::IO(ref e) => Some(e),
			Error::Poisoned(_) => None,
			Error::Queue(_) => None,
			Error::ValueTooLong => None,
		}
	}
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		use std::error::Error;
		write!(f, "Hammersbald error: {} cause: {:?}", self.description(), self.source())
	}
}

impl fmt::Debug for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		(self as &dyn fmt::Display).fmt(f)
	}
}

impl convert::From<io::Error> for Error {
	fn from(err: io::Error) -> Error {
		Error::IO(err)
	}
}

impl convert::From<Error> for io::Error {
	fn from(_: Error) -> io::Error {
		io::Error::from(io::ErrorKind::UnexpectedEof)
	}
}

impl<T> convert::From<sync::PoisonError<T>> for Error {
	fn from(err: sync::PoisonError<T>) -> Error {
		Error::Poisoned(err.to_string())
	}
}

impl<T> convert::From<sync::mpsc::SendError<T>> for Error {
	fn from(err: sync::mpsc::SendError<T>) -> Error {
		Error::Queue(err.to_string())
	}
}
