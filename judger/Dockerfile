FROM rust:1-slim-bookworm

ENV LANG en.UTF-8
RUN apt update
RUN apt install -y python3 libseccomp-dev gcc g++ # seccomp_sys
RUN useradd -s /sbin/nologin -u 2000 test

ADD ./src /app/src
ADD ./Cargo.toml /app/Cargo.toml
WORKDIR /app/
RUN cargo build --release
RUN mkdir /work
RUN mkdir /work/a
RUN chmod 700 /work/a

ENTRYPOINT ["/app/target/release/judger"]
