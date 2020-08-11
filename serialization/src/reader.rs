use crate::compact_integer::CompactInteger;
use std::cmp::min;
use std::{io, marker};

pub fn deserialize<R, T>(buffer: R) -> Result<T, Error>
where
	R: io::Read,
	T: Deserializable,
{
	let mut reader = Reader::from_read(buffer);
	let result = reader.read()?;

	if reader.is_finished() {
		Ok(result)
	} else {
		Err(Error::UnreadData)
	}
}

pub fn deserialize_iterator<R, T>(buffer: R) -> ReadIterator<R, T>
where
	R: io::Read,
	T: Deserializable,
{
	ReadIterator {
		reader: Reader::from_read(buffer),
		iter_type: marker::PhantomData,
	}
}

#[derive(Debug, PartialEq)]
pub enum Error {
	MalformedData,
	UnexpectedEnd,
	UnreadData,
}

impl Error {
	fn description(&self) -> &str {
		match *self {
			Error::MalformedData => "malformed data",
			Error::UnexpectedEnd => "unexpected end",
			Error::UnreadData => "unread data",
		}
	}
}

impl std::fmt::Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Deserialisation error: {}", &self.description())
	}
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
	fn from(_: io::Error) -> Self {
		Error::UnexpectedEnd
	}
}

pub trait Deserializable {
	fn deserialize<T>(reader: &mut Reader<T>) -> Result<Self, Error>
	where
		Self: Sized,
		T: io::Read;
}

/// Bitcoin structures reader.
#[derive(Debug)]
pub struct Reader<T> {
	buffer: T,
	peeked: Vec<u8>,
}

impl<'a> Reader<&'a [u8]> {
	/// Convenient way of creating for slice of bytes
	pub fn new(buffer: &'a [u8]) -> Self {
		Reader {
			buffer,
			peeked: Vec::new(),
		}
	}
}

impl<T> io::Read for Reader<T>
where
	T: io::Read,
{
	fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
		if buf.is_empty() {
			Ok(0)
		} else if self.peeked.is_empty() {
			io::Read::read(&mut self.buffer, buf)
		} else {
			let mut wrote = min(buf.len(), self.peeked.len());
			for i in 0..wrote {
				buf[i] = self.peeked.remove(i);
			}
			if buf.len() > wrote {
				wrote += io::Read::read(&mut self.buffer, &mut buf[wrote..])?;
			}
			Ok(wrote)
		}
	}
}

impl<R> Reader<R>
where
	R: io::Read,
{
	pub fn from_read(read: R) -> Self {
		Reader {
			buffer: read,
			peeked: Vec::new(),
		}
	}

	pub fn read<T>(&mut self) -> Result<T, Error>
	where
		T: Deserializable,
	{
		T::deserialize(self)
	}

	pub fn read_with_proxy<T, F>(&mut self, proxy: F) -> Result<T, Error>
	where
		T: Deserializable,
		F: FnMut(&[u8]),
	{
		let mut reader = Reader::from_read(Proxy::new(self, proxy));
		T::deserialize(&mut reader)
	}

	pub fn skip_while(&mut self, predicate: &dyn Fn(u8) -> bool) -> Result<(), Error> {
		let mut next_buffer = [0u8];

		loop {
			let next = if self.peeked.is_empty() {
				match self.buffer.read(&mut next_buffer)? {
					0 => return Ok(()),
					_ => next_buffer[0],
				}
			} else {
				self.peeked.remove(0)
			};

			if !predicate(next) {
				return Ok(());
			}
		}
	}

	pub fn read_slice(&mut self, bytes: &mut [u8]) -> Result<(), Error> {
		io::Read::read_exact(self, bytes).map_err(|_| Error::UnexpectedEnd)
	}

	pub fn read_list<T>(&mut self) -> Result<Vec<T>, Error>
	where
		T: Deserializable,
	{
		let len: usize = self.read::<CompactInteger>()?.into();
		let mut result = Vec::with_capacity(len);

		for _ in 0..len {
			result.push(self.read()?);
		}

		Ok(result)
	}

	pub fn read_list_max<T>(&mut self, max: usize) -> Result<Vec<T>, Error>
	where
		T: Deserializable,
	{
		let len: usize = self.read::<CompactInteger>()?.into();
		if len > max {
			return Err(Error::MalformedData);
		}

		let mut result = Vec::with_capacity(len);

		for _ in 0..len {
			result.push(self.read()?);
		}

		Ok(result)
	}

	pub fn peek(&mut self, buf: &mut [u8]) -> Result<(), Error> {
		if !self.peeked.is_empty() {
			return Err(Error::UnreadData);
		}

		return if self.read_slice(buf).is_ok() {
			self.peeked.extend_from_slice(buf);
			Ok(())
		} else {
			Err(Error::UnexpectedEnd)
		};
	}

	#[cfg_attr(feature = "cargo-clippy", allow(wrong_self_convention))]
	pub fn is_finished(&mut self) -> bool {
		if !self.peeked.is_empty() {
			return false;
		}

		let peek = &mut [0u8; 1];
		match self.read_slice(peek) {
			Ok(_) => {
				self.peeked = Vec::from(peek.as_ref());
				false
			}
			Err(_) => true,
		}
	}
}

/// Should be used to iterate over structures of the same type
pub struct ReadIterator<R, T> {
	reader: Reader<R>,
	iter_type: marker::PhantomData<T>,
}

impl<R, T> Iterator for ReadIterator<R, T>
where
	R: io::Read,
	T: Deserializable,
{
	type Item = Result<T, Error>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.reader.is_finished() {
			None
		} else {
			Some(self.reader.read())
		}
	}
}

struct Proxy<F, T> {
	from: F,
	to: T,
}

impl<F, T> Proxy<F, T> {
	fn new(from: F, to: T) -> Self {
		Proxy { from, to }
	}
}

impl<F, T> io::Read for Proxy<F, T>
where
	F: io::Read,
	T: FnMut(&[u8]),
{
	fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
		let len = io::Read::read(&mut self.from, buf)?;
		let to = &mut self.to;
		to(&buf[..len]);
		Ok(len)
	}
}
