FROM rust:1-alpine3.20

ENV LANG en.UTF-8
RUN ln -s /usr/share/zoneinfo/Asia/Shanghai /etc/localtime 
RUN echo 'Asia/Shanghai' > /etc/timezone
Run apk add musl-dev # compile serde_derive and clap_derive
Run apk add libseccomp-dev # seccomp_sys
RUN apk add python3 # checker
Run apk add gcc g++ # c/cpp support
RUN adduser -D -H -s /sbin/nologin -u 2000 test

ADD ./src /app/src
ADD ./Cargo.toml /app/Cargo.toml
WORKDIR /app/
RUN cargo build --release
RUN mkdir /work
RUN mkdir /work/a
RUN chmod 700 /work/a

ENTRYPOINT ["/app/target/release/judger"]
