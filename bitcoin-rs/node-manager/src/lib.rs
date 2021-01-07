mod node_manager;

pub use crate::node_manager::{NodeManager, SubCommand};

#[macro_export]
macro_rules! node {
	() => {
		NodeManager::new_test_node(
			env!("CARGO_BIN_EXE_bitcoin-rs"),
			env!("CARGO_CRATE_NAME"),
			env!("CARGO_MANIFEST_DIR"),
			)
	};
}
