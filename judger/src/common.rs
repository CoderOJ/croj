pub type Timestamp = chrono::DateTime<chrono::Utc>;
pub const TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.3fZ";

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

pub mod judger {
	use {
		crate::workaround,
		serde::{Deserialize, Serialize},
	};

	#[derive(Serialize, Deserialize, Debug, Clone)]
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
		pub uid:          u64,
		pub score:        f64,
		pub time_limit:   u64,
		pub memory_limit: u64,
		pub dependency:   Vec<u64>,
		pub pack_score:   f64,
	}

	/// Judge request data besides in/ans files
	#[derive(Serialize, Deserialize, Debug)]
	pub struct Request {
		pub code:    Code,
		pub sandbox: bool,
		pub cases:   Vec<Case>,
		pub checker: workaround::RemoteCommand,
	}

	// use french word resultat to differ from rust Result
	#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
	pub enum Resultat {
		Waiting,
		Running,
		Skipped,
		Accepted,
		#[serde(rename = "Compilation Error")]
		CompilationError,
		#[serde(rename = "Compilation Success")]
		CompilationSuccess,
		#[serde(rename = "Wrong Answer")]
		WrongAnswer,
		#[serde(rename = "Runtime Error")]
		RuntimeError,
		#[serde(rename = "Time Limit Exceeded")]
		TimeLimitExceeded,
		#[serde(rename = "Memory Limit Exceeded")]
		MemoryLimitExceeded,
		#[serde(rename = "System Error")]
		SystemError,
		#[serde(rename = "SPJ Error")]
		SPJError,
	}
	impl Resultat {
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
		pub result: Resultat,
		pub time:   u64,
		pub memory: u64,
		pub info:   String,
	}
	impl CaseResultInfo {
		pub fn skipped() -> Self {
			Self {
				result: Resultat::Skipped,
				time:   0,
				memory: 0,
				info:   String::new(),
			}
		}
	}

	#[derive(Serialize, Deserialize, Debug)]
	pub enum CaseResult {
		Waiting,
		Running,
		Skipped,
		Finished(CaseResultInfo),
	}

	/// Judge update: once per case
	#[derive(Serialize, Deserialize, Debug)]
	pub enum Update {
		Case(u64, CaseResult),
		/// General result update (e.g. compile)
		Compile(CaseResult),
		/// Finish(result, score)
		Finish(Resultat, f64),
		/// Internal Error
		Error(String),
	}
}
