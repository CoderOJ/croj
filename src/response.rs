use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct Error {
	pub code:    u64,
	pub reason:  String,
	pub message: String,
}
