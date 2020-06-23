extern crate built;

use std::path::PathBuf;
use std::process::Command;

// TODO put this somewhere better. own crate?
fn main() {
	// Setting up git hooks
	let git_hooks = format!("git config core.hooksPath {}", PathBuf::from("./.hooks").to_str().unwrap());

	if cfg!(target_os = "windows") {
		Command::new("cmd")
			.args(&["/C", &git_hooks])
			.output()
			.expect("failed to execute git config for hooks");
	} else {
		Command::new("sh")
			.args(&["-c", &git_hooks])
			.output()
			.expect("failed to execute git config for hooks");
	}
}
