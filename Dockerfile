FROM rust:1-alpine3.21 AS builder
RUN apk add --no-cache musl-dev
WORKDIR /oximqtt
COPY . .
RUN cargo build --release


FROM alpine:3.21
LABEL maintainer="oximqtt <oximqttd@126.com>"

RUN mkdir -p /app/oximqtt/oximqtt-bin
COPY --from=builder /oximqtt/target/release/oximqttd /app/oximqtt/oximqtt-bin/
COPY oximqtt.toml /app/oximqtt/
COPY oximqtt-bin/oximqtt.pem  /app/oximqtt/oximqtt-bin/
COPY oximqtt-bin/oximqtt.key  /app/oximqtt/oximqtt-bin/

WORKDIR /app/oximqtt

VOLUME ["/var/log/oximqtt"]

# oximqtt will occupy these port:
# - 1883 port for MQTT
# - 8883 port for MQTT(TLS)
# - 11883 port for internal MQTT/TCP
# - 6060 for APIs
EXPOSE 1883 8883 11883 6060

ENTRYPOINT ["sh", "-c", "/app/oximqtt/oximqtt-bin/oximqttd \"$@\"", "--"]

