/// Verbose block information
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct MemoryInfo {
	/// Number of bytes in use (in active pages) by the application.
	pub used: usize,
	/// Number of allocated bytes not currently in use.
	pub free: usize,
	/// Total number of bytes allocated to the application.
	pub total: usize,
}
