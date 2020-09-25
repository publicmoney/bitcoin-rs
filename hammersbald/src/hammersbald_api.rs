use crate::data_file::{DataFile, EnvelopeIterator};
use crate::format::{Envelope, Payload};
use crate::log_file::LogFile;
use crate::mem_table::MemTable;
use crate::pref::PRef;
use crate::stats;
use crate::table_file::TableFile;
use crate::Error;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::{
	io,
	io::{Cursor, Read, Write},
};

/// Hammersbald
pub struct Hammersbald {
	path: Option<String>,
	mem: MemTable,
}

/// public API to Hammersbald
pub trait HammersbaldAPI: Send + Sync {
	/// end current batch and start a new batch
	fn batch(&mut self) -> Result<(), Error>;

	/// stop background writer
	fn shutdown(&mut self) -> Result<(), Error>;

	/// store data accessible with key
	/// returns a persistent reference to stored data
	fn put_keyed(&mut self, key: &[u8], data: &[u8]) -> Result<u64, Error>;

	/// retrieve data with key
	/// returns Some(persistent reference, data) or None
	fn get_keyed(&self, key: &[u8]) -> Result<Option<(u64, Vec<u8>)>, Error>;

	/// store data
	/// returns a persistent reference
	fn put(&mut self, data: &[u8]) -> Result<u64, Error>;

	/// retrieve data using a persistent reference
	/// returns (key, data)
	fn get(&self, pref: u64) -> Result<(Vec<u8>, Vec<u8>), Error>;

	/// Update data at pref
	/// returns same pref or error
	fn set(&mut self, pref: u64, data: &[u8]) -> Result<u64, Error>;

	/// a quick (in-memory) check if the db may have the key
	/// this might return false positive, but if it is false key is definitely not used.
	fn may_have_key(&self, key: &[u8]) -> Result<bool, Error>;

	/// forget a key (if known)
	/// This is not a real delete as data will be still accessible through its PRef, but contains hash table growth
	fn forget(&mut self, key: &[u8]) -> Result<(), Error>;

	/// iterator of data
	fn iter(&self) -> HammersbaldIterator;

	/// print database stats
	fn stats(&self);

	fn size(&self) -> u64;
}

/// A helper to build Hammersbald data elements
pub struct HammersbaldDataWriter {
	data: Vec<u8>,
}

impl HammersbaldDataWriter {
	/// create a new builder
	pub fn new() -> HammersbaldDataWriter {
		HammersbaldDataWriter { data: vec![] }
	}

	/// serialized data
	pub fn as_slice<'a>(&'a self) -> &'a [u8] {
		self.data.as_slice()
	}

	/// append a persistent reference
	pub fn write_ref(&mut self, pref: PRef) {
		self.data.write_u48::<BigEndian>(pref.as_u64()).unwrap();
	}

	/// return a reader
	pub fn reader<'a>(&'a self) -> Cursor<&'a [u8]> {
		Cursor::new(self.data.as_slice())
	}
}

impl Write for HammersbaldDataWriter {
	fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
		self.data.write(buf)
	}

	fn flush(&mut self) -> Result<(), io::Error> {
		Ok(())
	}
}

/// Helper to read Hammersbald data elements
pub struct HammersbaldDataReader<'a> {
	reader: Cursor<&'a [u8]>,
}

impl<'a> HammersbaldDataReader<'a> {
	/// create a new reader
	pub fn new(data: &'a [u8]) -> HammersbaldDataReader<'a> {
		HammersbaldDataReader { reader: Cursor::new(data) }
	}

	/// read a persistent reference
	pub fn read_ref(&mut self) -> Result<PRef, io::Error> {
		Ok(PRef::from(self.reader.read_u48::<BigEndian>()?))
	}
}

impl<'a> Read for HammersbaldDataReader<'a> {
	fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
		self.reader.read(buf)
	}
}

impl Hammersbald {
	/// create a new db with key and data file
	pub fn new(path: &str, log: LogFile, table: TableFile, data: DataFile, link: DataFile) -> Result<Hammersbald, Error> {
		let mem = MemTable::new(log, table, data, link);
		let mut db = Hammersbald {
			path: Some(path.to_string()),
			mem,
		};
		db.recover()?;
		db.load()?;
		db.batch()?;
		Ok(db)
	}

	/// load memtable
	fn load(&mut self) -> Result<(), Error> {
		self.mem.load()
	}

	fn recover(&mut self) -> Result<(), Error> {
		self.mem.recover()
	}

	/// get hash table bucket iterator
	pub fn slots<'a>(&'a self) -> impl Iterator<Item = Vec<(u32, PRef)>> + 'a {
		self.mem.slots()
	}

	/// get hash table pointers
	pub fn buckets<'a>(&'a self) -> impl Iterator<Item = PRef> + 'a {
		self.mem.buckets()
	}

	/// return an iterator of all payloads
	pub fn data_envelopes<'a>(&'a self) -> impl Iterator<Item = (PRef, Envelope)> + 'a {
		self.mem.data_envelopes()
	}

	/// return an iterator of all links
	pub fn link_envelopes<'a>(&'a self) -> impl Iterator<Item = (PRef, Envelope)> + 'a {
		self.mem.link_envelopes()
	}

	/// get db params
	pub fn params(&self) -> (usize, u32, usize, u64, u64, u64, u64, u64) {
		self.mem.params()
	}
}

impl HammersbaldAPI for Hammersbald {
	fn batch(&mut self) -> Result<(), Error> {
		self.mem.batch()
	}

	fn shutdown(&mut self) -> Result<(), Error> {
		self.mem.shutdown()
	}

	fn put_keyed(&mut self, key: &[u8], data: &[u8]) -> Result<u64, Error> {
		#[cfg(debug_assertions)]
		{
			if key.len() > 255 || data.len() >= 1 << 23 {
				return Err(Error::KeyTooLong);
			}
		}
		if let Some((pref, current_data)) = self.mem.get(key)? {
			if current_data.len() == data.len() {
				self.mem.set(pref, data)?;
				Ok(pref.as_u64())
			} else {
				let data_offset = self.mem.append_data(key, data)?;
				self.mem.update_key(key, data_offset)?;
				Ok(data_offset.as_u64())
			}
		} else {
			let data_offset = self.mem.append_data(key, data)?;
			self.mem.put(key, data_offset)?;
			Ok(data_offset.as_u64())
		}
	}

	fn get_keyed(&self, key: &[u8]) -> Result<Option<(u64, Vec<u8>)>, Error> {
		self.mem.get(key).map(|r| r.map(|o| (o.0.as_u64(), o.1)))
	}

	fn put(&mut self, data: &[u8]) -> Result<u64, Error> {
		let data_offset = self.mem.append_referred(data)?;
		Ok(data_offset.as_u64())
	}

	fn get(&self, pref: u64) -> Result<(Vec<u8>, Vec<u8>), Error> {
		match self.mem.get_envelope(pref.into())?.payload()? {
			Payload::Referred(referred) => return Ok((vec![], referred.data.to_vec())),
			Payload::Indexed(indexed) => return Ok((indexed.key.to_vec(), indexed.data.data.to_vec())),
			_ => Err(Error::Corrupted("referred should point to data".to_string())),
		}
	}

	fn set(&mut self, pref: u64, data: &[u8]) -> Result<u64, Error> {
		let data_offset = self.mem.set(pref.into(), data)?;
		Ok(data_offset.as_u64())
	}

	fn may_have_key(&self, key: &[u8]) -> Result<bool, Error> {
		self.mem.may_have_key(key)
	}

	fn forget(&mut self, key: &[u8]) -> Result<(), Error> {
		self.mem.forget(key)
	}

	fn iter(&self) -> HammersbaldIterator {
		HammersbaldIterator {
			ei: self.mem.data_envelopes(),
		}
	}

	fn stats(&self) {
		stats::stats(self)
	}

	fn size(&self) -> u64 {
		match &self.path {
			Some(path) => std::fs::read_dir(path)
				.unwrap()
				.filter_map(|entry| entry.ok())
				.filter_map(|entry| entry.metadata().ok())
				.filter(|metadata| metadata.is_file())
				.fold(0, |acc, m| acc + m.len()),
			None => 0,
		}
	}
}

/// iterate data content
pub struct HammersbaldIterator<'a> {
	ei: EnvelopeIterator<'a>,
}

impl<'a> Iterator for HammersbaldIterator<'a> {
	type Item = (PRef, Vec<u8>, Vec<u8>);

	fn next(&mut self) -> Option<<Self as Iterator>::Item> {
		if let Some((pref, envelope)) = self.ei.next() {
			match envelope.payload().unwrap() {
				Payload::Indexed(indexed) => return Some((pref, indexed.key.to_vec(), indexed.data.data.to_vec())),
				Payload::Referred(referred) => return Some((pref, vec![], referred.data.to_vec())),
				_ => return None,
			}
		}
		None
	}
}
