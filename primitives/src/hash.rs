#[macro_export]
macro_rules! impl_array_wrapper {
	($name: ident, $size: expr) => {
		use std::hash::{Hash, Hasher};
		use std::{cmp, ops};

		#[repr(C)]
		#[derive(Copy, Debug)]
		pub struct $name(pub [u8; $size]);

		impl Default for $name {
			fn default() -> Self {
				$name([0u8; $size])
			}
		}

		impl AsRef<$name> for $name {
			fn as_ref(&self) -> &$name {
				self
			}
		}

		impl Clone for $name {
			fn clone(&self) -> Self {
				let mut result = Self::default();
				result.copy_from_slice(&self.0);
				result
			}
		}

		impl From<[u8; $size]> for $name {
			fn from(h: [u8; $size]) -> Self {
				$name(h)
			}
		}

		impl From<$name> for [u8; $size] {
			fn from(h: $name) -> Self {
				h.0
			}
		}

		impl<'a> From<&'a [u8]> for $name {
			fn from(slc: &[u8]) -> Self {
				let mut inner = [0u8; $size];
				inner[..].clone_from_slice(&slc[0..$size]);
				$name(inner)
			}
		}

		impl ops::Deref for $name {
			type Target = [u8; $size];

			fn deref(&self) -> &Self::Target {
				&self.0
			}
		}

		impl ops::DerefMut for $name {
			fn deref_mut(&mut self) -> &mut Self::Target {
				&mut self.0
			}
		}

		impl cmp::PartialEq for $name {
			fn eq(&self, other: &Self) -> bool {
				let self_ref: &[u8] = &self.0;
				let other_ref: &[u8] = &other.0;
				self_ref == other_ref
			}
		}

		impl cmp::PartialOrd for $name {
			fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
				let self_ref: &[u8] = &self.0;
				let other_ref: &[u8] = &other.0;
				self_ref.partial_cmp(other_ref)
			}
		}

		impl Hash for $name {
			fn hash<H>(&self, state: &mut H)
			where
				H: Hasher,
			{
				state.write(&self.0);
				state.finish();
			}
		}

		impl Eq for $name {}
	};
}
