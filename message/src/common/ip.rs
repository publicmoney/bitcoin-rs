use byteorder::{BigEndian, WriteBytesExt};
use ser::{Deserializable, Error as ReaderError, Reader, Serializable, Stream};
use std::convert::TryInto;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::{io, net, str};

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct IpAddress(net::IpAddr);

impl Default for IpAddress {
	fn default() -> Self {
		IpAddress(net::IpAddr::V4(net::Ipv4Addr::new(0, 0, 0, 0)))
	}
}

impl From<net::IpAddr> for IpAddress {
	fn from(ip: net::IpAddr) -> Self {
		IpAddress(ip)
	}
}

impl From<IpAddress> for net::IpAddr {
	fn from(ip: IpAddress) -> Self {
		ip.0
	}
}

impl From<&'static str> for IpAddress {
	fn from(s: &'static str) -> Self {
		s.parse().unwrap()
	}
}

impl str::FromStr for IpAddress {
	type Err = <net::IpAddr as str::FromStr>::Err;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		s.parse().map(IpAddress)
	}
}

impl Serializable for IpAddress {
	fn serialize(&self, stream: &mut Stream) {
		match self.0 {
			net::IpAddr::V4(address) => {
				stream.append_slice(&[0u8; 12]).append_slice(&address.octets());
			}
			net::IpAddr::V6(address) => {
				for segment in &address.segments() {
					stream.write_u16::<BigEndian>(*segment).unwrap();
				}
			}
		}
	}
}

impl Deserializable for IpAddress {
	fn deserialize<T>(reader: &mut Reader<T>) -> Result<Self, ReaderError>
	where
		T: io::Read,
	{
		let bytes: &mut [u8] = &mut [0u8; 16];
		reader.read_slice(bytes)?;

		if &bytes[0..12] == &[0u8; 12] {
			let address: &[u8; 4] = &bytes[12..16].try_into().unwrap();
			let ipv4 = Ipv4Addr::from(*address);
			Ok(IpAddress(net::IpAddr::V4(ipv4)))
		} else {
			let address: &[u8; 16] = &bytes[..].try_into().unwrap();
			let ipv6 = Ipv6Addr::from(*address);
			Ok(IpAddress(net::IpAddr::V6(ipv6)))
		}
	}
}

#[cfg(test)]
mod test {
	use super::IpAddress;
	use ser::{deserialize, serialize};
	use std::net;

	#[test]
	fn test_ip_serialize() {
		let ip = IpAddress(net::IpAddr::V6("::ffff:a00:1".parse().unwrap()));
		assert_eq!(
			serialize(&ip),
			vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0x0a, 0x00, 0x00, 0x01].into()
		);
		let ip = IpAddress(net::IpAddr::V4("10.0.0.1".parse().unwrap()));
		assert_eq!(
			serialize(&ip),
			vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x01].into()
		);
	}

	#[test]
	fn test_ip_deserialize() {
		let ip: IpAddress = deserialize(&[
			0x00u8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0x0a, 0x00, 0x00, 0x01,
		] as &[u8])
		.unwrap();
		assert_eq!(ip, IpAddress(net::IpAddr::V6("::ffff:a00:1".parse().unwrap())));
		let ip: IpAddress = deserialize(&[
			0x00u8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x01,
		] as &[u8])
		.unwrap();
		assert_eq!(ip, IpAddress(net::IpAddr::V4("10.0.0.1".parse().unwrap())));
	}
}
