use std::fmt;

#[derive(Debug, PartialEq)]
pub enum Error {
	DatabaseError(String),
	CannotCanonize,
	CannotDecanonize,
	UnknownParent,
	AncientFork,
	InconsistentData,
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Error::DatabaseError(s) => write!(f, "Database error: {}", s),
			Error::CannotCanonize => write!(f, "Cannot canonize block"),
			Error::CannotDecanonize => write!(f, "Cannot decanonize block"),
			Error::UnknownParent => write!(f, "Block parent is unknown"),
			Error::AncientFork => write!(f, "Fork is too long to proceed"),
			Error::InconsistentData => write!(f, "Database inconsistency detected"),
		}
	}
}
