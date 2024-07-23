use {
	crate::{
		callcc::{callcc, callcc_ret, KEntrance},
		response, user,
	},
	actix_web::{get, post, web, HttpResponse},
	serde::Deserialize,
	serde_json::json,
};

#[derive(Deserialize)]
struct Request {
	pub id:   Option<u64>,
	pub name: String,
}

#[post("/users")]
fn post(req: web::Json<Request>) -> KEntrance<HttpResponse> {
	let Request {
		id,
		name,
	} = req.into_inner();
	callcc_ret(move |k| {
		match id {
			// create user
			None => match user::new_user(name) {
				Ok(user) => {
					k.resume(HttpResponse::Ok().json(json!({
					  "id": user.id,
					  "name": &user.name,
					})));
				}
				Err(err) => {
					k.resume(HttpResponse::BadRequest().json(response::Error {
						reason:  "ERR_INVALID_ARGUMENT".to_string(),
						code:    1,
						message: err,
					}));
				}
			},
			// update user
			Some(id) => {
				let user = user::get_list_id()
					.get(id as usize)
					.ok_or(HttpResponse::NotFound().json(response::Error {
						reason:  "ERR_NOT_FOUND".to_string(),
						code:    3,
						message: format!("User {} not found.", id),
					}))?
					.clone();
				let new_user = user::update_user(user, name).map_err(|err| {
					HttpResponse::BadRequest().json(response::Error {
						reason:  "ERR_INVALID_ARGUMENT".to_string(),
						code:    1,
						message: err,
					})
				})?;
				k.resume(HttpResponse::Ok().json(json!({
				  "id": new_user.id,
				  "name": &new_user.name,
				})));
			}
		}
		return Ok(());
	})
}

#[get("/users")]
fn get() -> KEntrance<HttpResponse> {
	callcc(move |k| {
		k.resume(HttpResponse::Ok().json(json!(
      user::get_list_id().iter().map(|user| {
        json!({
          "id": user.id,
          "name": &user.name,
        })
      }).collect::<Vec<_>>()
    )));
	})
}
