use {
	judger::{config::*, judger::*, workaround::RemoteCommand},
	serde_json::to_string,
};

fn main() {
	println!(
		"{}",
		to_string(&Request {
			code:    Code {
				language: Language {
					name:      "Rust".to_string(),
					file_name: "main.rs".to_string(),
					command:   ["rustc", "-C", "opt-level=2", "-o", "%OUTPUT%", "%INPUT%"]
						.iter()
						.map(|s| s.to_string())
						.collect(),
				},
				source:   std::fs::read_to_string("tests/echo.rs").unwrap(),
			},
			sandbox: false,
			cases:   (0..2)
				.map(|id| Case {
					score:        50.0,
					input_file:   format!("{}", id),
					answer_file:  format!("{}", id),
					time_limit:   1_000_000 + id * 10_000_000,
					memory_limit: 64 * 1048576,
				})
				.collect(),
			checker: RemoteCommand::pack(
				vec!["python3", "../checkers/standard.py", "%OUTPUT%", "%ANSWER%"]
					.iter()
					.map(|s| s.to_string())
					.collect()
			),
		})
		.unwrap()
	);
}
