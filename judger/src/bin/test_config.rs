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
					name:      "C".to_string(),
					file_name: "main.c".to_string(),
					command:   ["gcc", "-o", "%OUTPUT%", "%INPUT%"]
						.iter()
						.map(|s| s.to_string())
						.collect(),
				},
				source:   std::fs::read_to_string("tests/read_ans.c").unwrap(),
			},
			sandbox: false,
			cases:   (0..2)
				.map(|id| Case {
					uid:          id as u64,
					score:        50.0,
					time_limit:   1_000_000 + id * 10_000_000,
					memory_limit: 64 * 1048576,
					dependency:   Vec::new(),
					pack_score:   50.0,
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
