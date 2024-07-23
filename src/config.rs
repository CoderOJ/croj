	use {
		super::judger,
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
		pub languages: Vec<judger::Language>,
	}

	pub struct Problem {
		pub id:       u64,
		pub name:     String,
		pub checker:  workaround::RemoteCommand,
		pub data_dir: String,
		pub cases:    Vec<judger::Case>,
		pub sandbox:  bool,
	}
	impl Problem {
		fn from(data_dir: &std::path::Path, raw: RawProblem) -> Result<Self> {
			fn parse_packing(
				packing: Option<Vec<Vec<u64>>>,
				mut cases: Vec<judger::Case>,
			) -> Result<Vec<judger::Case>> {
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
						.map(|(id, raw)| -> Result<_> {
							// this part should use crate::Fs feature instead of string concat
							std::fs::copy(raw.input_file, data_dir.join(format!("in{}", id)))?;
							std::fs::copy(raw.answer_file, data_dir.join(format!("ans{}", id)))?;
							return Ok(judger::Case {
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
		pub languages: HashMap<String, Arc<judger::Language>>,
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