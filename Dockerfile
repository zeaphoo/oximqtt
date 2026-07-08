# syntax=docker/dockerfile:1

# ── Stage 1: Build ──────────────────────────────────────────────
FROM rust:1-alpine3.21 AS builder

RUN apk add --no-cache musl-dev

WORKDIR /oximqtt
COPY . .
RUN cargo build --release

# ── Stage 2: Binary export (for --output extraction) ───────────
FROM scratch AS binaries
COPY --from=builder /oximqtt/target/release/oximqttd /

# ── Stage 3: Runtime ────────────────────────────────────────────
FROM alpine:3.21

LABEL maintainer="oximqtt <zeaphoo@qq.com>"

RUN mkdir -p /app/oximqtt/oximqtt-bin
COPY --from=builder /oximqtt/target/release/oximqttd /app/oximqtt/oximqtt-bin/
COPY oximqtt.toml /app/oximqtt/
COPY oximqtt-bin/oximqtt.pem  /app/oximqtt/oximqtt-bin/
COPY oximqtt-bin/oximqtt.key  /app/oximqtt/oximqtt-bin/

WORKDIR /app/oximqtt

VOLUME ["/var/log/oximqtt"]

# 1883  - MQTT/TCP
# 8883  - MQTT/TLS
# 8080  - MQTT/WebSocket
# 8443  - MQTT/WebSocket-TLS
# 9443  - MQTT/QUIC (UDP)
# 11883 - internal MQTT/TCP
EXPOSE 1883 8883 8080 8443 9443/udp 11883

ENTRYPOINT ["sh", "-c", "/app/oximqtt/oximqtt-bin/oximqttd \"$@\"", "--"]
