pub mod workaround {
	use {
		anyhow::{anyhow, Result},
		serde::{Deserialize, Serialize},
		std::io::Read,
	};

	#[derive(Serialize, Deserialize, Debug)]
	enum RemouteResource {
		String(String),
		File(String),
	}

	pub type Command = Vec<String>;

	#[derive(Serialize, Deserialize, Debug)]
	pub struct RemoteCommand {
		command: Vec<RemouteResource>,
	}

	impl RemoteCommand {
		pub fn pack(command: Vec<String>) -> Self {
			RemoteCommand {
				command: command
					.into_iter()
					.map(|entry| match std::fs::File::open(&entry) {
						Err(_) => RemouteResource::String(entry),
						Ok(mut file) => {
							let mut buf: String = "".to_string();
							file.read_to_string(&mut buf).unwrap();
							RemouteResource::File(buf)
						}
					})
					.collect(),
			}
		}

		pub fn unpack(self, mut gen: impl Iterator<Item = String>) -> Result<Vec<String>> {
			self.command
				.into_iter()
				.map(|entry| match entry {
					RemouteResource::String(str) => Ok(str),
					RemouteResource::File(content) => {
						let id = gen
							.next()
							.ok_or(anyhow!("RemoteCommand unpack: generator reaches end"))?;
						std::fs::write(&id, content)?;
						return Ok(id);
					}
				})
				.collect()
		}
	}
}

pub mod config {
	use serde::{Deserialize, Serialize};

	#[derive(Serialize, Deserialize, Debug)]
	pub struct Language {
		pub name:      String,
		pub file_name: String,
		pub command:   Vec<String>,
	}

	#[derive(Serialize, Deserialize, Debug)]
	pub struct Code {
		pub language: Language,
		pub source:   String,
	}

	#[derive(Serialize, Deserialize, Debug)]
	pub struct Case {
		pub score:        f64,
		pub input_file:   String,
		pub answer_file:  String,
		pub time_limit:   u64,
		pub memory_limit: u64,
	}
}

pub mod judger {
	use {
		crate::{config, workaround},
		serde::{Deserialize, Serialize},
	};

	/// Judge request data besides in/ans files
	#[derive(Serialize, Deserialize, Debug)]
	pub struct Request {
		pub code:    config::Code,
		pub cases:   Vec<config::Case>,
		pub checker: workaround::RemoteCommand,
	}

	#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
	pub enum JudgeResult {
		Waiting,
		Running,
		Skipped,
		Accepted,
		CompilationError,
		CompilationSuccess,
		WrongAnswer,
		RuntimeError,
		TimeLimitExceeded,
		MemoryLimitExceeded,
		SystemError,
		SPJError,
	}
	impl JudgeResult {
		pub fn score_coef(self) -> f64 {
			match self {
				Self::Accepted => 1.0,
				_ => 0.0,
			}
		}
		pub fn or(self, other: Self) -> Self {
			match (self, other) {
				(Self::Accepted, rhs) => rhs,
				(lhs, _) => lhs,
			}
		}
	}

	#[derive(Serialize, Deserialize, Debug)]
	pub struct CaseResultInfo {
		pub result: JudgeResult,
		pub time:   u64,
		pub memory: u64,
		pub info:   String,
	}

	#[derive(Serialize, Deserialize, Debug)]
	pub enum CaseResult {
		Waiting,
		Running,
		Skipped,
		Finished(CaseResultInfo),
	}

	#[derive(Serialize, Deserialize, Debug)]
	pub struct UpdateCase {
		pub id:   u64,
		pub data: CaseResult,
	}

	/// Judge update: once per case
	#[derive(Serialize, Deserialize, Debug)]
	pub enum Update {
		Case(UpdateCase),
		/// General result update (e.g. compile)
		General(JudgeResult),
		/// Finish(result, score)
		Finish(JudgeResult, f64),
		/// Internal Error
		Error(String),
	}
}
