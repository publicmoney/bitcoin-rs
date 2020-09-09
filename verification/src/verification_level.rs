#[derive(Debug, Clone, Copy, PartialEq)]
/// Blocks verification level.
pub enum VerificationLevel {
	/// Full verification.
	Full,
	/// Transaction scripts are not checked.
	Header,
	/// No verification at all.
	NoVerification,
}

impl Default for VerificationLevel {
	fn default() -> Self {
		VerificationLevel::Full
	}
}
