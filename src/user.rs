use {
	lazy_static::lazy_static,
	std::{
		collections::HashMap,
		sync::{Arc, Mutex, MutexGuard},
	},
};

type UserRef = Arc<User>;
pub struct User {
	pub id:   u64,
	pub name: String,
}

lazy_static! {
	static ref USER_ROOT: UserRef = Arc::new(User {
		id:   0,
		name: "root".to_string(),
	});
	static ref USER_LIST_ID: Arc<Mutex<Vec<UserRef>>> =
		Arc::new(Mutex::new(vec![USER_ROOT.clone(),]));
	static ref USER_LIST_NAME: Arc<Mutex<HashMap<String, UserRef>>> = Arc::new(Mutex::new(
		USER_LIST_ID
			.lock()
			.unwrap()
			.iter()
			.map(|user| (user.name.clone(), user.clone()))
			.collect()
	));
}

pub fn get_list_id() -> MutexGuard<'static, Vec<UserRef>> {
	USER_LIST_ID.lock().unwrap()
}

pub fn new_user(name: String) -> Result<UserRef, String> {
	let mut list_name = USER_LIST_NAME.lock().unwrap();
	match list_name.get(&name) {
		Some(_) => return Err(format!("User name '{}' already exists.", name)),
		None => {
			let mut list_id = USER_LIST_ID.lock().unwrap();
			let user = Arc::new(User {
				id:   list_id.len() as u64,
				name: name.clone(),
			});
			list_id.push(user.clone());
			list_name.insert(name, user.clone());
			return Ok(user);
		}
	}
}

pub fn update_user(user: UserRef, name: String) -> Result<UserRef, String> {
	let mut list_name = USER_LIST_NAME.lock().unwrap();
	match list_name.get(&name) {
		Some(_) => return Err(format!("User name '{}' already exists.", name)),
		None => {
			let new_user = Arc::new(User {
				id:   user.id,
				name: name.clone(),
			});
      let mut list_id = USER_LIST_ID.lock().unwrap();
      list_id[user.id as usize] = new_user.clone();
			list_name.remove(&new_user.name);
			list_name.insert(name, new_user.clone());
			return Ok(new_user);
		}
	}
}
