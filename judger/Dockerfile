FROM rust:1-alpine3.20
MAINTAINER CoderOJ

ENV LANG en.UTF-8
RUN ln -s /usr/share/zoneinfo/Asia/Shanghai /etc/localtime 
RUN echo 'Asia/Shanghai' > /etc/timezone

ADD ./ /app/
WORKDIR /app/
RUN cargo build

EXPOSE 8100
ENTRYPOINT ["cargo", "run"]
