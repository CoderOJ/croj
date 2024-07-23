use {
	crate::{callcc::*, common, config, judger, response, service, user},
	actix_web::{
		delete, get, post, put,
		web::{self},
		HttpResponse,
	},
	chrono::NaiveDateTime,
	cond::cond,
	serde::{Deserialize, Deserializer, Serialize},
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
	pub result: judger::Resultat,
	pub time:   u64,
	pub memory: u64,
	pub info:   String,
}

impl ResponseCase {
	fn from_case(id: u64, case: &judger::CaseResult) -> Self {
		let null = |result| Self {
			id,
			result,
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
				result: info.result,
				time: info.time,
				memory: info.memory,
				info: info.info.clone(),
			},
		}
	}
}

fn submission_to_response(result: service::SubmissionRef) -> serde_json::Value {
	let result = result.lock().unwrap();
	let cases = std::iter::once(&result.result_compile)
		.chain(result.result_cases.iter())
		.enumerate()
		.map(|(id, case)| ResponseCase::from_case(id as u64, case))
		.collect::<Vec<_>>();
	json!({
		"id": result.id,
		"created_time": result.created_time.format(common::TIME_FORMAT).to_string(),
		"updated_time": result.updated_time.format(common::TIME_FORMAT).to_string(),
		"submission": result.raw.as_ref(),
		"state": result.state,
		"result":result.result_final,
		"score": result.score,
		"cases": cases,
	})
}

#[post("/jobs")]
fn post(req: web::Json<Submission>, config: web::Data<config::Config>) -> KEntrance<HttpResponse> {
	let submission = Arc::new(req.into_inner());
	callcc_ret(move |k: KEntrance<HttpResponse>| {
		let _user = user::get_list_id()
			.get(submission.user_id as usize)
			.ok_or(HttpResponse::NotFound().json(response::Error {
				code:    3,
				reason:  "ERR_NOT_FOUND".to_string(),
				message: format!("User {:?} not found", &submission.user_id),
			}))?;

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

		let result = service::new_job(request);
		k.resume(HttpResponse::Ok().json(submission_to_response(result)));

		return Ok(());
	})
}

#[put("/jobs/{id}")]
fn put_id(id: web::Path<u64>) -> KEntrance<HttpResponse> {
	return callcc(move |k: KEntrance<HttpResponse>| {
		let list = service::get_list();
		let id = id.into_inner() as usize;
		cond! {
			id >= list.len() => k.resume(HttpResponse::NotFound().json(response::Error {
				code: 3,
				reason: "ERR_NOT_FOUND".to_string(),
				message: format!("Job {} not found.", id),
			})),
			list[id].lock().unwrap().state != service::SubmissionState::Finished => {
				k.resume(HttpResponse::BadRequest().json(response::Error {
					code: 2,
					reason: "ERR_INVALID_STATE".to_string(),
					message: format!("Job {} not finished.", id),
				}));
			},
			_ => {
				let submission = service::rerun_job(list[id].clone());
				k.resume(HttpResponse::Ok().json(
					submission_to_response(submission)
				));
			},
		}
	});
}

#[delete("/jobs/{id}")]
fn delete_id(id: web::Path<u64>) -> KEntrance<HttpResponse> {
	return callcc(move |k: KEntrance<HttpResponse>| {
		let list = service::get_list();
		let id = id.into_inner() as usize;
		cond! {
			id >= list.len() => k.resume(HttpResponse::NotFound().json(response::Error {
				code: 3,
				reason: "ERR_NOT_FOUND".to_string(),
				message: format!("Job {} not found.", id),
			})),
			_ => {
				match service::cancel_job(list[id].clone()) {
					Err(())	=> k.resume(HttpResponse::BadRequest().json(response::Error {
						code: 2,
						reason: "ERR_INVALID_STATE".to_string(),
						message: format!("Job {} not queueing.", id),
					})),
					Ok(()) => k.resume(HttpResponse::Ok().body("")),
				}
			},
		}
	});
}

#[get("/jobs/{id}")]
fn get_id(id: web::Path<u64>) -> KEntrance<HttpResponse> {
	return callcc(move |k: KEntrance<HttpResponse>| {
		let list = service::get_list();
		let id = id.into_inner() as usize;
		cond! {
			id >= list.len() => k.resume(HttpResponse::NotFound().json(response::Error {
				code: 3,
				reason: "ERR_NOT_FOUND".to_string(),
				message: format!("Job {} not found.", id),
			})),
			_ => k.resume(HttpResponse::Ok().json(
				submission_to_response(list[id].clone())
			)),
		}
	});
}

// adapted from: https://serde.rs/custom-date-format.html
fn deserialize_option_time<'de, D>(deserializer: D) -> Result<Option<common::Timestamp>, D::Error>
where
	D: Deserializer<'de>,
{
	let s = String::deserialize(deserializer)?;
	let dt =
		NaiveDateTime::parse_from_str(&s, common::TIME_FORMAT).map_err(serde::de::Error::custom)?;
	Ok(Some(common::Timestamp::from_naive_utc_and_offset(
		dt,
		chrono::Utc,
	)))
}

#[derive(Deserialize, Debug)]
struct GetParam {
	problem_id: Option<u64>,
	language:   Option<String>,
	#[serde(default, deserialize_with = "deserialize_option_time")]
	from:       Option<common::Timestamp>,
	to:         Option<common::Timestamp>,
	state:      Option<service::SubmissionState>,
	result:     Option<judger::Resultat>,
}

#[get("/jobs")]
fn get(req: web::Query<GetParam>) -> KEntrance<HttpResponse> {
	fn option_filter<T>(f: impl Fn(&T, &T) -> bool, a: &Option<T>, b: &T) -> bool {
		a.as_ref().map_or(true, |a| f(a, b))
	}
	fn equal<T: PartialEq>(a: &T, b: &T) -> bool {
		a == b
	}
	let req = req.into_inner();
	let filter = move |submission: &service::SubmissionRef| -> bool {
		let submission = submission.lock().unwrap();
		option_filter(equal, &req.problem_id, &submission.problem.id)
			&& option_filter(equal, &req.language, &submission.language.name)
			&& option_filter(equal, &req.state, &submission.state)
			&& option_filter(equal, &req.result, &submission.result_final)
			&& option_filter(|a, b| a <= b, &req.from, &submission.created_time)
			&& option_filter(|a, b| a >= b, &req.to, &submission.created_time)
	};

	return callcc(move |k: KEntrance<HttpResponse>| {
		k.resume(
			HttpResponse::Ok().json(
				service::get_list()
					.iter()
					.cloned()
					.filter(filter)
					.map(submission_to_response)
					.collect::<Vec<_>>(),
			),
		);
	});
}
