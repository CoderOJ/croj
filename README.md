# CROJ

CROJ is a distributed Online Judge (OJ) system written in Rust, designed for secure, scalable, and efficient code evaluation.

## Features

+ 🚀 Distributed Judging: Leveraging Docker to run code evaluation tasks across distributed nodes.

+ 🔒 Secure Execution: Enforces strict resource constraints using seccomp and setrlimit, ensuring isolation and safety.

+ 🦀 Built in Rust: High performance, memory safety, and robust concurrency.

## Highlights

+ Language agnostic: Supports judging submissions in multiple languages via containerized environments.

+ Horizontal scalability: Easily add judge nodes to scale with load.

+ Fine-grained sandboxing: Seccomp and setrlimit enforce CPU, memory, and syscall restrictions per submission.

+ Modular design: Cleanly separated components for scheduler, judge daemon, submission frontend, etc.
