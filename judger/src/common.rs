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

pub mod config {
	use {
		crate::workaround,
		serde::{Deserialize, Serialize},
		std::{
			collections::HashMap,
			fs,
			io::{Error, Result},
			sync::Arc,
		},
	};

	#[derive(Serialize, Deserialize, Debug, Clone)]
	pub struct Language {
		pub name:      String,
		pub file_name: String,
		pub command:   Vec<String>,
	}

	#[derive(Serialize, Deserialize, Debug, Clone)]
	pub struct RawCase {
		pub score:        f64,
		pub input_file:   String,
		pub answer_file:  String,
		pub time_limit:   u64,
		pub memory_limit: u64,
	}

	#[derive(Serialize, Deserialize, Debug)]
	pub struct Server {
		pub bind_address: String,
		pub bind_port:    u16,
	}

	#[derive(Serialize, Deserialize, Debug)]
	pub enum RawProblemType {
		#[serde(rename = "standard")]
		Standard,
		#[serde(rename = "strict")]
		Strict,
		#[serde(rename = "spj")]
		Checker,
	}

	#[derive(Serialize, Deserialize, Debug)]
	pub struct RawProblemMisc {
		pub special_judge: Option<Vec<String>>,
		pub packing:       Option<Vec<Vec<u64>>>,
	}

	#[derive(Serialize, Deserialize, Debug)]
	pub struct RawProblem {
		pub id:      u64,
		pub name:    String,
		#[serde(rename = "type")]
		pub type_:   RawProblemType,
		pub misc:    RawProblemMisc,
		pub cases:   Vec<RawCase>,
		pub sandbox: Option<bool>,
	}

	#[derive(Serialize, Deserialize, Debug)]
	pub struct RawConfig {
		pub server:    Server,
		pub problems:  Vec<RawProblem>,
		pub languages: Vec<Language>,
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

	pub struct Problem {
		pub id:       u64,
		pub name:     String,
		pub checker:  workaround::RemoteCommand,
		pub data_dir: String,
		pub cases:    Vec<Case>,
		pub sandbox:  bool,
	}
	impl Problem {
		fn from(data_dir: &std::path::Path, raw: RawProblem) -> Result<Self> {
			fn parse_packing(
				packing: Option<Vec<Vec<u64>>>,
				mut cases: Vec<Case>,
			) -> Result<Vec<Case>> {
				match packing {
					None => {
						for case in &mut cases {
							case.pack_score = case.score;
						}
						Ok(cases)
					}
					Some(packing) => {
						for mut pack in packing {
							for uid in &mut pack {
								*uid -= 1;
							}
							let &last_uid = pack.last().ok_or(Error::other("ill packing"))?;
							let mut prev_uid: Option<u64> = None;
							for uid in pack {
								cases[last_uid as usize].pack_score += cases[uid as usize].score;
								if let Some(prev_uid) = prev_uid {
									cases[uid as usize].dependency.push(prev_uid);
								}
								prev_uid = Some(uid);
							}
						}
						Ok(cases)
					}
				}
			}

			Ok(Self {
				id:       raw.id,
				name:     raw.name,
				checker:  workaround::RemoteCommand::pack(match raw.type_ {
					RawProblemType::Standard => {
						["python3", "./checkers/standard.py", "%OUTPUT%", "%ANSWER%"]
							.map(String::from)
							.to_vec()
					}
					RawProblemType::Strict => {
						["python3", "./checkers/strict.py", "%OUTPUT%", "%ANSWER%"]
							.map(String::from)
							.to_vec()
					}
					RawProblemType::Checker => raw.misc.special_judge.as_ref().unwrap().clone(),
				}),
				data_dir: data_dir.to_str().unwrap().to_string(),
				cases:    parse_packing(
					raw.misc.packing,
					raw.cases
						.into_iter()
						.enumerate()
						.map(|(id, raw)| -> Result<Case> {
							// this part should use crate::Fs feature instead of string concat
							std::fs::copy(raw.input_file, data_dir.join(format!("in{}", id)))?;
							std::fs::copy(raw.answer_file, data_dir.join(format!("ans{}", id)))?;
							return Ok(Case {
								uid:          id as u64,
								score:        raw.score,
								time_limit:   raw.time_limit,
								memory_limit: match raw.memory_limit {
									// max(configurable) memory limit: 2G
									0 => 2 * 1024 * 1024 * 1024,
									x => x,
								},
								dependency:   Vec::new(),
								pack_score:   0.0,
							});
						})
						.collect::<Result<_>>()?,
				)?,
				sandbox:  raw.sandbox.unwrap_or(false),
			})
		}
	}

	pub struct Config {
		pub server:    Server,
		pub problems:  HashMap<u64, Arc<Problem>>,
		pub languages: HashMap<String, Arc<Language>>,
	}

	impl Config {
		pub fn from(data_dir: &std::path::Path, raw_config: RawConfig) -> Result<Config> {
			Ok(Config {
				server:    raw_config.server,
				problems:  raw_config
					.problems
					.into_iter()
					.map(|raw| -> Result<(u64, Arc<Problem>)> {
						let problem_dir = data_dir.join(format!("{}", raw.id));
						fs::create_dir_all(&problem_dir).unwrap();
						return Ok((raw.id, Arc::new(Problem::from(&problem_dir, raw)?)));
					})
					.collect::<Result<HashMap<u64, Arc<Problem>>>>()?,
				languages: raw_config
					.languages
					.into_iter()
					.map(|p| (p.name.clone(), Arc::new(p)))
					.collect(),
			})
		}
	}
}

pub mod judger {
	use {
		crate::{config, workaround},
		serde::{Deserialize, Serialize},
	};

	#[derive(Serialize, Deserialize, Debug)]
	pub struct Code {
		pub language: config::Language,
		pub source:   String,
	}

	/// Judge request data besides in/ans files
	#[derive(Serialize, Deserialize, Debug)]
	pub struct Request {
		pub code:    Code,
		pub sandbox: bool,
		pub cases:   Vec<config::Case>,
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
