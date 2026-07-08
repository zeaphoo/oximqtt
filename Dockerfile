FROM rust:1-alpine3.21 AS builder
RUN apk add --no-cache musl-dev
WORKDIR /oximqtt
COPY . .
RUN cargo build --release


FROM alpine:3.21
LABEL maintainer="oximqtt <zeaphoo@qq.com>"

RUN mkdir -p /app/oximqtt/oximqtt-bin
COPY --from=builder /oximqtt/target/release/oximqttd /app/oximqtt/oximqtt-bin/
COPY oximqtt.toml /app/oximqtt/
COPY oximqtt-bin/oximqtt.pem  /app/oximqtt/oximqtt-bin/
COPY oximqtt-bin/oximqtt.key  /app/oximqtt/oximqtt-bin/

WORKDIR /app/oximqtt

VOLUME ["/var/log/oximqtt"]

# oximqtt will occupy these ports:
# - 1883  for MQTT/TCP
# - 8883  for MQTT/TLS
# - 8080  for MQTT/WebSocket
# - 8443  for MQTT/WebSocket-TLS
# - 9443  for MQTT/QUIC (UDP)
# - 11883 for internal MQTT/TCP
EXPOSE 1883 8883 8080 8443 9443/udp 11883

ENTRYPOINT ["sh", "-c", "/app/oximqtt/oximqtt-bin/oximqttd \"$@\"", "--"]

