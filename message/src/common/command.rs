use crate::Error as MessageError;
use bitcrypto::FromHex;
use ser::{Deserializable, Error, Reader};
use ser::{Serializable, Stream};

impl_array_wrapper!(Command, 12);
impl_ser_for_array!(Command, 12);

impl std::str::FromStr for Command {
	type Err = MessageError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		if !s.is_ascii() || s.len() > 12 {
			return Err(MessageError::InvalidCommand);
		}

		let mut result = Command::default();
		result[..s.len()].copy_from_slice(s.as_ref());
		Ok(result)
	}
}

impl From<&'static str> for Command {
	fn from(s: &'static str) -> Self {
		s.parse().unwrap()
	}
}

impl Command {
	pub fn from_hex(s: &str) -> Result<Self, MessageError> {
		Ok(Command(FromHex::from_hex(s).map_err(|_| MessageError::InvalidCommand)?))
	}

	pub fn len(&self) -> usize {
		let trailing_zeros = self.0.iter().rev().take_while(|&x| x == &0).count();
		self.0.len() - trailing_zeros
	}

	fn as_string(&self) -> String {
		String::from_utf8_lossy(&self.0[..self.len()]).to_ascii_lowercase()
	}
}

impl From<Command> for String {
	fn from(c: Command) -> Self {
		c.as_string()
	}
}

impl std::fmt::Display for Command {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		f.write_str(&self.as_string())
	}
}

impl<'a> PartialEq<&'a str> for Command {
	fn eq(&self, other: &&'a str) -> bool {
		self.len() == other.len() && &self.0[..other.len()] == other.as_ref() as &[u8]
	}
}

#[cfg(test)]
mod tests {
	use super::Command;
	use crate::bytes::Bytes;
	use ser::{deserialize, serialize};

	#[test]
	fn test_command_parse() {
		let command: Command = "version".into();
		assert_eq!(Command::from_hex("76657273696f6e0000000000").unwrap(), command);
	}

	#[test]
	fn test_command_to_string() {
		let command: Command = "version".into();
		let expected: String = "version".into();
		assert_eq!(expected, String::from(command));
	}

	#[test]
	fn test_command_serialize() {
		let expected: Bytes = "76657273696f6e0000000000".into();
		let command: Command = "version".into();

		assert_eq!(serialize(&command), expected);
	}

	#[test]
	fn test_command_deserialize() {
		let raw: Bytes = "76657273696f6e0000000000".into();
		let expected: Command = "version".into();

		assert_eq!(expected, deserialize::<_, Command>(raw.as_ref()).unwrap());
	}

	#[test]
	fn partial_eq_command_str() {
		let command: Command = "version".into();
		assert_eq!(command, "version");
		assert!(command != "ver");
		assert!(command != "versionx");
	}
}
