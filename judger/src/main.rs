// judger service
// this code is written in principle that each statement is either:
// 1. let
// 2. cps style, (pure) side effect, which is either:
//   + run command: compile/main/diff
//   + send update

use {
	anyhow::{anyhow, Error, Result},
	cond::cond,
	judger::{config::*, fs::Fs, judger::*, workaround},
	serde_json::{from_str, to_string},
	std::{
		io::Read,
		os::unix::process::ExitStatusExt,
		process::{Child, Command, ExitStatus, Stdio},
		time::{Duration, Instant},
	},
	wait4::{ResUse, ResourceUsage, Wait4},
};

fn try_catch<F: Fn() -> Result<()>, Reject: FnOnce(Error)>(f: F, reject: Reject) {
	match f() {
		Ok(_) => unreachable!(),
		Err(e) => reject(e),
	}
}

struct Usage {
	pub status: ExitStatus,
	pub time:   std::time::Duration,
	pub memory: u64,
}

trait WaitUsageTimeout {
	fn wait_usage_timeout(&mut self, timeout: std::time::Duration) -> Result<Usage>;
}

impl WaitUsageTimeout for Child {
	fn wait_usage_timeout(&mut self, timeout: std::time::Duration) -> Result<Usage> {
		let time_start = Instant::now();

		// user-time tle killer, in case use sleep for long
		// unix pid might be reused by future subprocess, causing killing wrong process
		// so a notifier is needed
		let (killer_send, killer_recv) = std::sync::mpsc::channel::<()>();
		let pid = self.id() as i32;
		std::thread::spawn(move || {
			std::thread::sleep(timeout);
			// if haven't receive "finish" signal
			if killer_recv.try_recv().is_err() {
				unsafe { libc::kill(pid, libc::SIGKILL) };
			}
		});

		let ResUse {
			status,
			rusage: ResourceUsage {
				utime: _,
				stime: _,
				maxrss: memory,
			},
		} = self.wait4()?;
		let time = time_start.elapsed();
		// disable killer
		let _ = killer_send.send(());

		return Ok(Usage {
			status,
			time,
			memory,
		});
	}
}

fn compile<F: FnMut(Update)>(fs: &Fs, code: &Code, mut send: F) -> Result<()> {
	send(Update::Compile(CaseResult::Running));
	let mut child = Command::new(&code.language.command[0])
		.args(
			code.language
				.command
				.iter()
				.skip(1)
				.map(|entry| match entry.as_str() {
					"%OUTPUT%" => fs.target.raw(),
					"%INPUT%" => fs.source.raw(),
					_ => entry,
				}),
		)
		.spawn()?;
	let Usage {
		status,
		time,
		memory,
	} = child.wait_usage_timeout(Duration::from_secs(10))?;

	match status.success() {
		true => {
			send(Update::Compile(CaseResult::Finished(CaseResultInfo {
				result: Resultat::CompilationSuccess,
				time: time.as_millis() as u64,
				memory,
				info: String::new(),
			})));
		}
		false => {
			send(Update::Compile(CaseResult::Finished(CaseResultInfo {
				result: Resultat::CompilationError,
				time: time.as_millis() as u64,
				memory,
				info: String::new(),
			})));
			send(Update::Finish(Resultat::CompilationError, 0.0));
		}
	}

	return Ok(());
}

fn run_case<F: FnMut(CaseResult)>(
	fs: &Fs,
	sandbox: bool,
	case: &Case,
	checker: &workaround::Command,
	mut send_case: F,
) -> Result<()> {
	send_case(CaseResult::Running);

	let input_file = &fs.input.at(case.uid);
	let output_file = &fs.output;
	let answer_file = &fs.answer.at(case.uid);

	let runner = format!(
		"{}/sandbox",
		std::env::var("JUDGER_BIN_DIR").unwrap_or("/app/target/release".to_string())
	);

	let mut child = Command::new(runner)
		.args(vec![
			"-r",
			&format!("./{}", fs.target.raw()),
			"-t",
			&format!("{}", case.time_limit),
			"-m",
			&format!("{}", case.memory_limit),
			"-s",
			&format!("{}", sandbox),
		])
		.stdin(Stdio::from(input_file.getter()?))
		.stdout(Stdio::from(output_file.setter()?))
		.stderr(Stdio::null())
		.spawn()?;

	let timeout = Duration::from_micros(case.time_limit + 1_000_000);
	let Usage {
		status,
		time,
		memory,
	} = child.wait_usage_timeout(timeout)?;
	let time = time.as_micros() as u64;

	send_case(CaseResult::Finished(match status.success() {
		// MLE, TLE, RE
		false => CaseResultInfo {
			result: cond! {
				memory > case.memory_limit => Resultat::MemoryLimitExceeded,
				time > case.time_limit => Resultat::TimeLimitExceeded,
				_ => Resultat::RuntimeError,
			},
			time,
			memory,
			info: match status.code() {
				None => match status.signal().unwrap() {
					31 => "Dangerous Syscall".to_string(),
					_ => format!("killed by signal {}", status.signal().unwrap()),
				},
				Some(code) => format!("exit with code {}", code),
			},
		},
		// AC, WA, SPJError
		true => {
			let mut checker_command_it = checker.into_iter();
			let mut checker_process =
				Command::new(checker_command_it.next().ok_or(anyhow!("empty spj"))?)
					.args(checker_command_it.map(|entry| match entry.as_str() {
						"%INPUT%" => input_file.raw(),
						"%OUTPUT%" => fs.output.raw(),
						"%ANSWER%" => answer_file.raw(),
						_ => entry,
					}))
					.stdin(Stdio::null())
					.stdout(Stdio::from(fs.checker_output.setter()?))
					.stderr(Stdio::null())
					.spawn()?;
			match checker_process
				.wait_usage_timeout(Duration::from_secs(1))?
				.status
				.success()
			{
				true => {
					let checker_output = fs.checker_output.get()?;
					let mut iter = checker_output.split("\n");
					let result_type = iter.next();
					let result_info = iter.next().unwrap_or("").to_string();
					match result_type {
						Some("Accepted") => CaseResultInfo {
							result: Resultat::Accepted,
							time,
							memory,
							info: result_info,
						},
						_ => CaseResultInfo {
							result: Resultat::WrongAnswer,
							time,
							memory,
							info: result_info,
						},
					}
				}
				false => CaseResultInfo {
					result: Resultat::SPJError,
					time,
					memory,
					info: "".to_string(),
				},
			}
		}
	}));
	return Ok(());
}

// send takes onwership as a continuation
fn send(data: Update) {
	println!("{}", to_string(&data).unwrap());

	// (only) exit continuation
	if let Update::Finish(_, _) = &data {
		std::process::exit(0);
	}
}

fn main() {
	// this try_catch env replace the main
	// reject: replace exitCode and stderr
	try_catch(
		|| {
			// work dir mapped by docker
			let fs = Fs::bind(&std::env::var("JUDGER_WORK_DIR").unwrap_or("/work".to_string()))?;

			let Request {
				cases,
				sandbox,
				code,
				checker,
			} = || -> Result<Request> {
				let mut buf: String = String::new();
				std::io::stdin().read_to_string(&mut buf)?;
				return Ok(from_str(&buf)?);
			}()?;

			// unpack checker
			let checker = checker.unpack(fs.checker.iter().map(|f| f.raw().clone()))?;

			// save & compile source
			fs.source.set(&code.source)?;
			compile(&fs, &code, send)?;

			// run cases
			let mut score: f64 = 0.0;
			let mut general_result = Resultat::Accepted;
			for (id, case) in cases.iter().enumerate() {
				run_case(&fs, sandbox, case, &checker, |data: CaseResult| {
					if let CaseResult::Finished(info) = &data {
						score += info.result.score_coef() * case.score;
						general_result = general_result.or(info.result);
					}
					send(Update::Case(id as u64, data));
				})?;
			}

			send(Update::Finish(general_result, score));

			return Err(anyhow!("judger reach end without sending Finish"));
		},
		|err| {
			send(Update::Error(err.to_string()));
		},
	);
}
