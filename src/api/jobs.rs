use {
	crate::{callcc::*, common, config, judger, response, service},
	actix_web::{get, post, web, HttpResponse},
	serde::{Deserialize, Serialize},
	serde_json::json,
	std::sync::Arc,
};

#[derive(Deserialize, Serialize, Clone)]
pub struct Submission {
	pub source_code: Arc<String>,
	pub language:    String,
	pub user_id:     u64,
	pub contest_id:  u64,
	pub problem_id:  u64,
}

#[derive(Serialize, Clone)]
pub struct ResponseCase {
	pub id:     u64,
	pub result: String,
	pub time:   u64,
	pub memory: u64,
	pub info:   String,
}

impl ResponseCase {
	fn from_case(id: u64, case: &judger::CaseResult) -> Self {
		let null = |result| Self {
			id,
			result: String::from(format_resultat(result)),
			time: 0,
			memory: 0,
			info: String::new(),
		};
		match case {
			judger::CaseResult::Waiting => null(judger::Resultat::Waiting),
			judger::CaseResult::Running => null(judger::Resultat::Running),
			judger::CaseResult::Skipped => null(judger::Resultat::Skipped),
			judger::CaseResult::Finished(info) => Self {
				id,
				result: String::from(format_resultat(info.result)),
				time: info.time,
				memory: info.memory,
				info: info.info.clone(),
			},
		}
	}
}

fn format_resultat(result: judger::Resultat) -> &'static str {
	match result {
		judger::Resultat::Waiting => "Waiting",
		judger::Resultat::Running => "Running",
		judger::Resultat::Skipped => "Skipped",
		judger::Resultat::Accepted => "Accepted",
		judger::Resultat::CompilationError => "Compilation Error",
		judger::Resultat::CompilationSuccess => "Compilation Success",
		judger::Resultat::WrongAnswer => "Wrong Answer",
		judger::Resultat::RuntimeError => "Runtime Error",
		judger::Resultat::TimeLimitExceeded => "Time Limit Exceeded",
		judger::Resultat::MemoryLimitExceeded => "Memory Limit Exceeded",
		judger::Resultat::SystemError => "System Error",
		judger::Resultat::SPJError => "SPJ Error",
	}
}

fn format_time(time: &common::Timestamp) -> String {
	time.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

fn result_response(result: service::ResultatRef) -> serde_json::Value {
	let result = result.lock().unwrap();
	let cases = std::iter::once(&result.result_compile)
		.chain(result.result_cases.iter())
		.enumerate()
		.map(|(id, case)| ResponseCase::from_case(id as u64, case))
		.collect::<Vec<_>>();
	json!({
		"id": result.id,
		"created_time": format_time(&result.created_time),
		"updated_time": format_time(&result.updated_time),
		"submission": result.submission.as_ref(),
		"state": result.state,
		"result": format_resultat(result.result_final),
		"score": result.score,
		"cases": cases,
	})
}

#[post("/jobs")]
fn post(req: web::Json<Submission>, config: web::Data<config::Config>) -> KEntrance<HttpResponse> {
	let submission = Arc::new(req.into_inner());
	callcc_ret(move |k: KEntrance<HttpResponse>| {
		let language = config
			.languages
			.get(&submission.language)
			.ok_or(HttpResponse::NotFound().json(response::Error {
				code:    3,
				reason:  "ERR_NOT_FOUND".to_string(),
				message: format!("language {:?} not found", &submission.language),
			}))?
			.clone();

		let problem = config
			.problems
			.get(&submission.problem_id)
			.ok_or(HttpResponse::NotFound().json(response::Error {
				code:    3,
				reason:  "ERR_NOT_FOUND".to_string(),
				message: format!("problem {:?} not found", &submission.problem_id),
			}))?
			.clone();

		let request = service::Request {
			source: submission.source_code.clone(),
			language,
			problem,
			submission,
			// cases:   problem.cases.clone(),
		};

		let result = service::exec(request);
		k.resume(HttpResponse::Ok().json(result_response(result)));

		return Ok(());
	})
}

#[get("/jobs/{id}")]
async fn get_id(id: web::Path<u64>) -> HttpResponse {
	HttpResponse::Ok().json(result_response(service::get_result(*id)))
}
