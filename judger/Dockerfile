FROM rust:1-alpine3.20

ENV LANG en.UTF-8
RUN ln -s /usr/share/zoneinfo/Asia/Shanghai /etc/localtime 
RUN echo 'Asia/Shanghai' > /etc/timezone
Run apk add musl-dev # compile serde_derive and clap_derive
Run apk add libseccomp-dev # seccomp_sys
RUN apk add python3
RUN adduser -D -H -s /sbin/nologin -u 2000 test

ADD ./ /app/
WORKDIR /app/
RUN cargo build --release

ENTRYPOINT ["cargo", "run"]
