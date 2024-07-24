use {
	clap::Parser,
	exec,
	rlimit::{setrlimit, Resource},
	seccomp_sys::*,
};

#[derive(Parser, Debug)]
struct Args {
	#[arg(short, long, default_value_t = String::from("main"))]
	run: String,

	/// time limit(us)
	#[arg(short, long)]
	time:   u64,
	/// memory limit(byte)
	#[arg(short, long)]
	memory: u64,

	/// enable sandbox
	#[arg(short, long)]
	sandbox: Option<bool>,
}

fn set_seccomp() {
	unsafe {
		let ctx = seccomp_init(SCMP_ACT_KILL_PROCESS);

		// allow sys_sigaltstack
		seccomp_rule_add(ctx, SCMP_ACT_ALLOW, 131, 0);

		// allow sys_exitgroup
		seccomp_rule_add(ctx, SCMP_ACT_ALLOW, 231, 0);

		// allow 0-8, 17-21, 262: read/write/state files
		for i in (0..9).chain(17..22).chain(262..263) {
			seccomp_rule_add(ctx, SCMP_ACT_ALLOW, i, 0);
		}

		// allow 9-12: memory
		for i in 9..13 {
			seccomp_rule_add(ctx, SCMP_ACT_ALLOW, i, 0);
		}

		// allow 158 ?
		seccomp_rule_add(ctx, SCMP_ACT_ALLOW, 158, 0);

		// allow 218 ?
		seccomp_rule_add(ctx, SCMP_ACT_ALLOW, 218, 0);

		// allow 257: opennat
		seccomp_rule_add(ctx, SCMP_ACT_ALLOW, 257, 0);

		// allow 257: readlinkat
		seccomp_rule_add(ctx, SCMP_ACT_ALLOW, 267, 0);

		// allow 273
		seccomp_rule_add(ctx, SCMP_ACT_ALLOW, 273, 0);

		// allow 302: prlimit
		seccomp_rule_add(ctx, SCMP_ACT_ALLOW, 302, 0);

		// allow 334 - 335 ?
		seccomp_rule_add(ctx, SCMP_ACT_ALLOW, 334, 0);
		seccomp_rule_add(ctx, SCMP_ACT_ALLOW, 335, 0);

		// allow next execve
		seccomp_rule_add(ctx, SCMP_ACT_ALLOW, 59, 0);

		// allow sys_getrandom
		seccomp_rule_add(ctx, SCMP_ACT_ALLOW, 318, 0);

		// for rust program
		seccomp_rule_add(ctx, SCMP_ACT_ALLOW, 13, 0);
		seccomp_rule_add(ctx, SCMP_ACT_ALLOW, 204, 0);

		seccomp_load(ctx);
	}
}

fn set_rlimit(args: &Args) {
	// rlimit time: round(time + 1)
	let time_sec = (args.time + 1_500_000) / 1_000_000;
	setrlimit(Resource::CPU, time_sec, time_sec).unwrap();

	// rlimit vmemory: memory + 64MiB
	let memory_byte = args.memory + 64 * 1048576;
	setrlimit(Resource::AS, memory_byte, memory_byte).unwrap();
	setrlimit(Resource::DATA, memory_byte, memory_byte).unwrap();
	setrlimit(Resource::STACK, memory_byte, memory_byte).unwrap();

	setrlimit(Resource::NPROC, 1, 1).unwrap();
}

fn main() {
	let args = Args::parse();

	unsafe {
		libc::setuid(2000);
		libc::setgid(2000);
	}
	set_rlimit(&args);
	if args.sandbox.unwrap_or(false) {
		set_seccomp();
	}

	let err = exec::Command::new(args.run).exec();
	match err {
		exec::Error::BadArgument(_) => {}
		exec::Error::Errno(errno) => std::process::exit(errno.0),
	}
}
