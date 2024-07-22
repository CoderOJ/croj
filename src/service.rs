/// judge queue handler
use {
	crate::{config, judger},
	anyhow::{anyhow, Result},
	chrono::Utc,
	lazy_static::lazy_static,
	serde::Serialize,
	serde_json::{from_str, json},
	std::{
		io::{BufRead, BufReader, Write},
		process::{Command, Stdio},
		sync::{
			mpsc::{Receiver, Sender},
			Arc, Mutex,
		},
	},
};

pub struct Request {
	pub source:     Arc<String>,
	pub language:   Arc<config::Language>,
	pub problem:    Arc<config::Problem>,
	pub submission: Arc<crate::api::jobs::Submission>,
}

#[derive(Serialize)]
pub enum ResultatState {
	Queueing,
	Running,
	Finished,
	Canceled,
	SystemError(String),
}

pub type ResultatRef = Arc<Mutex<Resultat>>;
pub struct Resultat {
	pub id:             u64,
	pub created_time:   crate::common::Timestamp,
	pub updated_time:   crate::common::Timestamp,
	pub submission:     Arc<crate::api::jobs::Submission>,
	pub state:          ResultatState,
	pub result_final:   judger::Resultat,
	pub result_compile: judger::CaseResult,
	pub result_cases:   Vec<judger::CaseResult>,
	pub score:          f64,
}

impl Resultat {
	fn new(
		id: u64,
		problem: &config::Problem,
		submission: Arc<crate::api::jobs::Submission>,
	) -> Self {
		Self {
			id,
			created_time: chrono::Utc::now(),
			updated_time: chrono::Utc::now(),
			submission,
			state: ResultatState::Queueing,
			result_final: judger::Resultat::Waiting,
			result_compile: judger::CaseResult::Waiting,
			score: 0.0,
			result_cases: problem
				.cases
				.iter()
				.map(|_| judger::CaseResult::Waiting)
				.collect(),
		}
	}
}

struct Job {
	result:  ResultatRef,
	request: Request,
}

struct JobRunner {
	send: Sender<Job>,
}

fn runner(cpuid: u8, recv: Receiver<Job>) {
	while let Ok(Job {
		result,
		request,
	}) = recv.recv()
	{
		log::debug!("grab test");

		// try_catch wrapper
		if let Err(err) = ({
			let result = result.clone();
			move || -> Result<()> {
				// main process
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
						// map data dir
						format!("-v=./{}:/work", &request.problem.data_dir).as_str(),
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
								"language": request.language.as_ref(),
								"source": &request.source,
							},
							"sandbox": request.problem.sandbox,
							"cases": &request.problem.cases,
							"checker": &request.problem.checker,
						})
						.to_string()
						.as_bytes(),
					)?;

				let mut recv =
					BufReader::new(child.stdout.take().ok_or(anyhow!("child has no stdout"))?)
						.lines();
				while let Some(Ok(update_str)) = recv.next() {
					let update: judger::Update = from_str(&update_str)?;
					let mut result = result.lock().unwrap();
					result.updated_time = Utc::now();
					match update {
						judger::Update::Compile(data) => {
							result.result_compile = data;
						}
						judger::Update::Case(id, data) => {
							result.result_cases[id as usize] = data;
						}
						judger::Update::Finish(cur, score) => {
							result.result_final = cur;
							result.score = score;
							result.state = ResultatState::Finished;
						}
						judger::Update::Error(err) => {
							result.state = ResultatState::SystemError(err);
						}
					}
				}

				let status = child.wait()?;
				if !status.success() {
					return Err(anyhow!("judger failed"));
				}

				// if judger hasn't send "Finish" before disconnect
				if !matches!(result.lock().unwrap().state, ResultatState::Finished) {
					return Err(anyhow!("judger disconnected"));
				}

				return Ok(());
			}
		})() {
			let mut result = result.lock().unwrap();
			result.updated_time = Utc::now();
			result.state = ResultatState::SystemError(err.to_string());
		}
	}
}

impl JobRunner {
	fn new() -> Self {
		let (send, recv) = std::sync::mpsc::channel::<Job>();
		std::thread::spawn(move || {
			runner(0, recv);
		});
		Self {
			send,
		}
	}
	fn send(&self, job: Job) {
		self.send.send(job).unwrap();
	}
}

lazy_static! {
	static ref RESULT_LIST: Arc<Mutex<Vec<ResultatRef>>> = Arc::new(Mutex::new(Vec::new()));
	static ref JOB_RUNNER: JobRunner = JobRunner::new();
}

pub fn exec(request: Request) -> ResultatRef {
	let result = {
		let mut job_list = RESULT_LIST.lock().unwrap();
		let job = Arc::new(Mutex::new(Resultat::new(
			job_list.len() as u64,
			&request.problem,
			request.submission.clone(),
		)));
		job_list.push(job.clone());
		job
	};

	JOB_RUNNER.send(Job {
		result: result.clone(),
		request,
	});

	return result;
}

pub fn get_result(id: u64) -> ResultatRef {
	let job_list = RESULT_LIST.lock().unwrap();
	job_list[id as usize].clone()
}
