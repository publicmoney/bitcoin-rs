use crate::error::Error;
use crate::pref::PRef;

use crate::memtable::BUCKET_FILL_TARGET;
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};

/// Content envelope wrapping in data file
#[derive(Debug)]
pub struct Envelope {
	buffer: Vec<u8>,
}

impl Envelope {
	pub fn from_payload(payload: Payload) -> Envelope {
		Envelope {
			buffer: payload.serialize(),
		}
	}

	/// envelope payload
	pub fn payload(&self) -> Result<Payload, Error> {
		Payload::deserialize(self.buffer.as_slice())
	}

	pub fn serialize(&self) -> Vec<u8> {
		let mut v = vec![];
		v.write_u24::<BigEndian>(self.buffer.len() as u32).unwrap();
		v.extend_from_slice(self.buffer.as_slice());
		v
	}

	/// deserialize for storage
	pub fn deseralize(buffer: Vec<u8>) -> Envelope {
		Envelope { buffer }
	}

	pub fn len(&self) -> usize {
		self.buffer.len()
	}
}

/// payloads in the data file
pub enum Payload<'e> {
	/// indexed data
	Indexed(IndexedData<'e>),
	/// data
	Referred(Data<'e>),
	/// hash table extension,
	Link(Link<'e>),
}

impl<'e> Payload<'e> {
	/// serialize for storage
	pub fn serialize(&self) -> Vec<u8> {
		let mut result = vec![];
		match self {
			Payload::Indexed(indexed) => {
				result.push(0);
				result.extend_from_slice(indexed.serialize().as_slice());
			}
			Payload::Referred(referred) => {
				result.push(1);
				result.extend_from_slice(referred.serialize().as_slice());
			}
			Payload::Link(link) => {
				result.push(2);
				result.extend_from_slice(link.serialize().as_slice());
			}
		};
		result
	}

	/// deserialize from storage
	pub fn deserialize(slice: &'e [u8]) -> Result<Payload, Error> {
		match slice[0] {
			0 => Ok(Payload::Indexed(IndexedData::deserialize(&slice[1..]))),
			1 => Ok(Payload::Referred(Data::deserialize(&slice[1..]))),
			2 => Ok(Payload::Link(Link::deserialize(&slice[1..]))),
			_ => Err(Error::Corrupted("unknown payload type".to_string())),
		}
	}

	pub fn set_data(&mut self, data: &'e [u8]) {
		match self {
			Payload::Indexed(indexed) => {
				indexed.data.data = data;
			}
			Payload::Referred(referred) => {
				referred.data = data;
			}
			_ => panic!("Links should not be updated"),
		};
	}

	pub fn into_envelope(self) -> Envelope {
		Envelope::from_payload(self)
	}
}

/// data that is accessible only if its position is known
pub struct Data<'e> {
	/// data
	pub data: &'e [u8],
}

impl<'e> Data<'e> {
	/// create new data
	pub fn new(data: &'e [u8]) -> Data<'e> {
		Data { data }
	}

	/// serialize for storage
	pub fn serialize(&self) -> Vec<u8> {
		let mut result = vec![];
		result.write_u24::<BigEndian>(self.data.len() as u32).unwrap();
		result.extend_from_slice(self.data);
		result
	}

	/// deserialize from storage
	pub fn deserialize(slice: &'e [u8]) -> Data {
		let data_len = BigEndian::read_u24(&slice[0..3]) as usize;
		let data = &slice[3..3 + data_len];
		Data { data }
	}

	pub fn into_payload(self) -> Payload<'e> {
		Payload::Referred(self)
	}
}

/// data accessible with a key
pub struct IndexedData<'e> {
	/// key
	pub key: &'e [u8],
	/// data
	pub data: Data<'e>,
}

impl<'e> IndexedData<'e> {
	/// new indexed data
	pub fn new(key: &'e [u8], data: Data<'e>) -> IndexedData<'e> {
		IndexedData { key, data }
	}

	/// serialize for storage
	pub fn serialize(&self) -> Vec<u8> {
		let mut result = vec![];
		result.push(self.key.len() as u8);
		result.extend_from_slice(self.key);
		result.extend_from_slice(self.data.serialize().as_slice());
		result
	}

	/// deserialize from storage
	pub fn deserialize(slice: &'e [u8]) -> IndexedData<'e> {
		let key_len = slice[0] as usize;
		let key = &slice[1..key_len + 1];
		let data = Data::deserialize(&slice[key_len + 1..]);
		IndexedData { key, data }
	}

	pub fn to_payload(self) -> Payload<'e> {
		Payload::Indexed(self)
	}
}

/// A link to data
pub struct Link<'e> {
	/// slots
	links: &'e [u8],
}

impl<'e> Link<'e> {
	/// serialize slots
	pub fn from_slots(slots: &[(u32, PRef)]) -> Vec<u8> {
		// Make the vec max size it will need to be so that it can be updated and not get overwritten.
		let mut links = vec![0u8; 10 * BUCKET_FILL_TARGET * 8];
		for (i, slot) in slots.iter().enumerate() {
			BigEndian::write_u32(&mut links[i * 10..i * 10 + 4], slot.0);
			BigEndian::write_u48(&mut links[i * 10 + 4..i * 10 + 10], slot.1.as_u64());
		}
		links
	}

	/// get slots
	pub fn slots(&self) -> Vec<(u32, PRef)> {
		let mut slots = vec![];
		for i in 0..self.links.len() / 10 {
			let hash = BigEndian::read_u32(&self.links[i * 10..i * 10 + 4]);
			let pref = PRef::from(BigEndian::read_u48(&self.links[i * 10 + 4..i * 10 + 10]));
			if hash > 0 && pref != PRef::invalid() {
				slots.push((hash, pref));
			}
		}
		slots
	}

	/// serialize for storage
	pub fn serialize(&self) -> Vec<u8> {
		self.links.to_vec()
	}

	/// deserialize from storage
	pub fn deserialize(slice: &'e [u8]) -> Link<'e> {
		Link { links: slice }
	}

	pub fn to_payload(self) -> Payload<'e> {
		Payload::Link(self)
	}
}
