use crate::error::Error;
use crate::format::{Envelope, Payload};
use crate::page::PAGE_SIZE;
use crate::paged_file::{PagedFile, PagedFileAppender};
use crate::pref::PRef;

use byteorder::{BigEndian, ByteOrder};

/// file storing indexed and referred data
pub struct DataFile {
	appender: PagedFileAppender,
}

impl DataFile {
	/// create new file
	pub fn new(file: Box<dyn PagedFile>) -> Result<DataFile, Error> {
		let len = file.len()?;
		if len >= PAGE_SIZE as u64 {
			Ok(DataFile {
				appender: PagedFileAppender::new(file, PRef::from(len)),
			})
		} else {
			let appender = PagedFileAppender::new(file, PRef::from(0));
			Ok(DataFile { appender })
		}
	}

	pub fn set_pos(&mut self, pos: PRef) {
		self.appender.set_pos(pos);
	}

	/// return an iterator of all payloads
	pub fn envelopes(&self) -> EnvelopeIterator {
		EnvelopeIterator::new(&self.appender)
	}

	/// shutdown
	pub fn shutdown(&mut self) -> Result<(), Error> {
		self.appender.shutdown()
	}

	/// get a stored content at pref
	pub fn get_envelope(&self, mut pref: PRef) -> Result<Envelope, Error> {
		let mut len = [0u8; 3];
		pref = self.appender.read(pref, &mut len)?;
		let len = BigEndian::read_u24(&len) as usize;
		let mut buf = vec![0u8; len];
		self.appender.read(pref, &mut buf)?;
		Ok(Envelope::deseralize(buf.to_vec()))
	}

	pub fn append(&mut self, payload: Payload) -> Result<PRef, Error> {
		let env = payload.into_envelope();
		let data = env.serialize();
		let me = self.appender.position();
		self.appender.append(data.as_slice())?;
		Ok(me)
	}

	pub fn update(&mut self, pref: PRef, payload: Payload) -> Result<PRef, Error> {
		let env = payload.into_envelope();
		let data = env.serialize();
		let me = self.appender.position();
		self.appender.update(pref, data.as_slice())?;
		Ok(me)
	}

	pub fn set_data(&mut self, pref: PRef, data: &[u8]) -> Result<PRef, Error> {
		let envelope = self.get_envelope(pref)?;

		let mut payload = envelope.payload()?;
		payload.set_data(data);
		let new_envelope = payload.into_envelope();

		if envelope.len() != new_envelope.len() {
			return Err(Error::ValueTooLong);
		}

		self.appender.update(pref, &new_envelope.serialize())?;
		Ok(pref)
	}

	/// truncate file
	pub fn truncate(&mut self, pref: u64) -> Result<(), Error> {
		self.appender.truncate(pref)
	}

	/// flush buffers
	pub fn flush(&mut self) -> Result<(), Error> {
		self.appender.flush()
	}

	/// sync file on file system
	pub fn sync(&self) -> Result<(), Error> {
		self.appender.sync()
	}

	/// get file length
	pub fn len(&self) -> Result<u64, Error> {
		self.appender.len()
	}
}

/// Iterate data file content
pub struct EnvelopeIterator<'f> {
	file: &'f PagedFileAppender,
	pos: PRef,
}

impl<'f> EnvelopeIterator<'f> {
	/// create a new iterator
	pub fn new(file: &'f PagedFileAppender) -> EnvelopeIterator<'f> {
		EnvelopeIterator { file, pos: PRef::from(0) }
	}
}

impl<'f> Iterator for EnvelopeIterator<'f> {
	type Item = (PRef, Envelope);

	fn next(&mut self) -> Option<<Self as Iterator>::Item> {
		if self.pos.is_valid() {
			let start = self.pos;
			let mut len = [0u8; 3];
			if let Ok(pos) = self.file.read(start, &mut len) {
				let length = BigEndian::read_u24(&len) as usize;
				if length > 0 {
					let mut buf = vec![0u8; length];
					self.pos = self.file.read(pos, &mut buf).unwrap();
					let envelope = Envelope::deseralize(buf);
					return Some((start, envelope));
				}
			}
		}
		None
	}
}
