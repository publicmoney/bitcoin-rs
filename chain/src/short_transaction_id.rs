use bitcrypto::{FromHex, HexError};
use primitives::impl_array_wrapper;
use ser::impl_ser_for_array;
use ser::{Deserializable, Error, Reader};
use ser::{Serializable, Stream};

impl_array_wrapper!(ShortTransactionID, 6);
impl_ser_for_array!(ShortTransactionID, 6);

impl ShortTransactionID {
	pub fn from_hex(s: &str) -> Result<Self, HexError> {
		Ok(ShortTransactionID(FromHex::from_hex(s)?))
	}
}
