/// judge queue handler
use {
	crate::{config, judger},
	anyhow::{anyhow, Result},
	chrono::Utc,
	cond::cond,
	lazy_static::lazy_static,
	serde::{Deserialize, Serialize},
	serde_json::{from_str, json},
	std::{
		io::{BufRead, BufReader, Write},
		process::{Command, Stdio},
		sync::{
			mpsc::{Receiver, Sender},
			Arc, Mutex, MutexGuard,
		},
	},
};

pub struct Request {
	pub source:     Arc<String>,
	pub language:   Arc<config::Language>,
	pub problem:    Arc<config::Problem>,
	pub submission: Arc<crate::api::jobs::Submission>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum SubmissionState {
	Queueing,
	Running,
	Finished,
	Canceled,
	SystemError(String),
}

pub type SubmissionRef = Arc<Mutex<Submission>>;
pub struct Submission {
	// info
	pub id:             u64,
	pub source:         Arc<String>,
	pub language:       Arc<config::Language>,
	pub problem:        Arc<config::Problem>,
	pub raw:            Arc<crate::api::jobs::Submission>,
	// result
	pub created_time:   crate::common::Timestamp,
	pub updated_time:   crate::common::Timestamp,
	pub state:          SubmissionState,
	pub result_final:   judger::Resultat,
	pub result_compile: judger::CaseResult,
	pub result_cases:   Vec<judger::CaseResult>,
	pub score:          f64,
}

impl Submission {
	fn new(id: u64, request: Request) -> Self {
		Self {
			id,
			// inital result
			created_time: chrono::Utc::now(),
			updated_time: chrono::Utc::now(),
			state: SubmissionState::Queueing,
			result_final: judger::Resultat::Waiting,
			result_compile: judger::CaseResult::Waiting,
			score: 0.0,
			result_cases: request
				.problem
				.cases
				.iter()
				.map(|_| judger::CaseResult::Waiting)
				.collect(),
			// info
			source: request.source,
			language: request.language,
			problem: request.problem,
			raw: request.submission,
		}
	}

	// before rerun job
	pub fn clear(&mut self) {
		self.state = SubmissionState::Queueing;
		self.result_final = judger::Resultat::Waiting;
		self.result_compile = judger::CaseResult::Waiting;
		for case in self.result_cases.iter_mut() {
			*case = judger::CaseResult::Waiting
		}
	}
}

fn runner(cpuid: u8, recv: Receiver<SubmissionRef>) {
	while let Ok(submission) = recv.recv() {
		log::debug!("grab test");

		// try_catch wrapper
		if let Err(err) = ({
			let submission = submission.clone();
			move || -> Result<()> {
				// main process
				{
					let mut submission = submission.lock().unwrap();
					if submission.state == SubmissionState::Canceled {
						return Ok(());
					} else {
						submission.state = SubmissionState::Running;
						submission.result_final = judger::Resultat::Running;
					}
				}

				// lock submission, start runner
				let (mut child, mut recv) = {
					let submission = submission.lock().unwrap();
					let mut child = Command::new("docker")
						.args([
							"run",
							// once container
							"--rm",
							// bind stdin
							"-i",
							// bind cpu
							format!("--cpuset-cpus={}", cpuid).as_str(),
							// memory limit
							"-m=2G",
							// no network access
							"--network=none",
							// map data dir ro
							format!("-v=./{}:/work/a/data:ro", &submission.problem.data_dir).as_str(),
							// start container
							"oj-judger",
						])
						.stdin(Stdio::piped())
						.stdout(Stdio::piped())
						.stderr(Stdio::null())
						.spawn()?;

					// transfer request to container via stdin
					child
						.stdin
						.take()
						.ok_or(anyhow!("child has no stdin"))?
						.write_all(
							json!({
								"code": {
									"language": submission.language.as_ref(),
									"source": &submission.source,
								},
								"sandbox": submission.problem.sandbox,
								"cases": &submission.problem.cases,
								"checker": &submission.problem.checker,
							})
							.to_string()
							.as_bytes(),
						)?;

					let recv =
						BufReader::new(child.stdout.take().ok_or(anyhow!("child has no stdout"))?)
							.lines();
					(child, recv)
				};

				while let Some(Ok(update_str)) = recv.next() {
					let update: judger::Update = from_str(&update_str)?;
					let mut submission = submission.lock().unwrap();
					submission.updated_time = Utc::now();
					match update {
						judger::Update::Compile(data) => {
							submission.result_compile = data;
						}
						judger::Update::Case(id, data) => {
							submission.result_cases[id as usize] = data;
						}
						judger::Update::Finish(cur, score) => {
							submission.result_final = cur;
							submission.score = score;
							submission.state = SubmissionState::Finished;
						}
						judger::Update::Error(err) => {
							submission.state = SubmissionState::SystemError(err);
						}
					}
				}

				let status = child.wait()?;

				return cond! {
					!status.success() => Err(anyhow!("judger failed")),
					!matches!(submission.lock().unwrap().state, SubmissionState::Finished) => Err(anyhow!("judger disconnected")),
					_ => Ok(()),
				};
			}
		})() {
			let mut submission = submission.lock().unwrap();
			submission.updated_time = Utc::now();
			submission.state = SubmissionState::SystemError(err.to_string());
		}
	}
}

struct JobRunner {
	send: Sender<SubmissionRef>,
}

impl JobRunner {
	fn new() -> Self {
		let (send, recv) = std::sync::mpsc::channel::<SubmissionRef>();
		std::thread::spawn(move || {
			runner(0, recv);
		});
		Self {
			send,
		}
	}
	fn send(&self, job: SubmissionRef) {
		self.send.send(job).unwrap();
	}
}

lazy_static! {
	static ref SUBMISSION_LIST: Arc<Mutex<Vec<SubmissionRef>>> = Arc::new(Mutex::new(Vec::new()));
	static ref JOB_RUNNER: JobRunner = JobRunner::new();
}

pub fn new_job(request: Request) -> SubmissionRef {
	let submission = {
		let mut job_list = SUBMISSION_LIST.lock().unwrap();
		let job = Arc::new(Mutex::new(Submission::new(job_list.len() as u64, request)));
		job_list.push(job.clone());
		job
	};

	JOB_RUNNER.send(submission.clone());
	return submission;
}

pub fn rerun_job(submission: SubmissionRef) -> SubmissionRef {
	submission.lock().unwrap().clear();
	JOB_RUNNER.send(submission.clone());
	return submission;
}

pub fn cancel_job(submission: SubmissionRef) -> Result<(), ()> {
	let mut submission = submission.lock().unwrap();
	return match submission.state {
		SubmissionState::Queueing => {
			submission.state = SubmissionState::Canceled;
			Ok(())
		}
		_ => Err(()),
	};
}

pub fn get_list() -> MutexGuard<'static, Vec<SubmissionRef>> {
	SUBMISSION_LIST.lock().unwrap()
}
