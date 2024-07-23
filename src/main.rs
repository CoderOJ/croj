use {
	actix_web::{middleware::Logger, post, web, App, HttpServer, Responder},
	clap::Parser,
	env_logger, log,
	oj::config::Config,
	serde_json::from_str,
};

#[derive(Parser, Debug)]
struct Args {
	#[arg(short, long)]
	config:     String,
	#[arg(short, long, default_value_t = false)]
	flush_data: bool,
}

// DO NOT REMOVE: used in automatic testing
#[post("/internal/exit")]
#[allow(unreachable_code)]
async fn exit() -> impl Responder {
	log::info!("Shutdown as requested");
	std::process::exit(0);
	format!("Exited")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
	let args = Args::parse();
	let data_dir = std::path::Path::new("./data");
	let config = web::Data::new(Config::from(
		&data_dir,
		from_str(&std::fs::read_to_string(args.config)?)?,
	)?);

	HttpServer::new({
		let config = config.clone();
		move || {
			App::new()
				.app_data(config.clone())
				.wrap(Logger::default())
				// DO NOT REMOVE: used in automatic testing
				.service(exit)
				.service(oj::api::jobs::post)
				.service(oj::api::jobs::put_id)
				.service(oj::api::jobs::delete_id)
				.service(oj::api::jobs::get)
				.service(oj::api::jobs::get_id)
				.service(oj::api::users::post)
				.service(oj::api::users::get)
		}
	})
	.bind((config.server.bind_address.as_str(), config.server.bind_port))?
	.run()
	.await
}
