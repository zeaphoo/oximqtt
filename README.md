# OXIMQTT Broker

[![GitHub Release](https://img.shields.io/github/release/oximqtt/oximqtt?color=brightgreen)](https://github.com/zeaphoo/oximqtt/releases)
[![Rust Version](https://img.shields.io/badge/rust-1.89.0%2B-blue)](https://blog.rust-lang.org/2025/08/07/Rust-1.89.0/)
[![crates.io](https://img.shields.io/crates/v/oximqtt.svg)](https://crates.io/crates/oximqtt)
[![docs.rs](https://docs.rs/oximqtt/badge.svg)](https://docs.rs/oximqtt/latest/oximqtt/)

[**English**](README.md) | [简体中文](README-CN.md)

A lightweight, high-performance MQTT broker written in 100% safe Rust. Built on [tokio](https://crates.io/crates/tokio), designed for IoT, M2M, and mobile applications.

## Features

- MQTT v3.1, v3.1.1, and v5.0 protocol support
- QoS 0, 1, 2 message delivery
- TLS / WebSocket / WebSocket-TLS / QUIC transports
- Built-in modules (configured in `oximqtt.toml`):
  - **ACL** — rule-based publish/subscribe authorization
  - **Retainer** — retained message storage
  - **JWT Auth** — JWT-based client authentication
  - **Sys Topic** — `$SYS` system topic publishing
- Exclusive and limited subscriptions
- Delayed publish (`$delayed/{Interval}/{Topic}`)
- Metrics and runtime stats
- Rate limiting, inflight window, and message queue
- Retained and Last Will messages
- Offline message queuing
- Library mode — embed as a Rust crate

## Quick Start

### Build from source

```bash
cargo build -p oximqttd --release
./target/release/oximqttd
```

### Use as a library

```toml
[dependencies]
oximqtt = "0.22"
```

See [oximqtt crate docs](./oximqtt/README.md) for library usage examples.

## Configuration

All configuration is in a single `oximqtt.toml` file. Key sections:

| Section | Description |
|---------|-------------|
| `[listener.*]` | TCP, TLS, WebSocket, WSS, QUIC listeners |
| `[acl]` | Access control rules |
| `[retainer]` | Retained message storage |
| `[auth_jwt]` | JWT authentication |
| `[sys_topic]` | System topic publishing |
| `[node]` | Node identity and cluster settings |
| `[log]` | Logging configuration |
| `[mqtt]` | Protocol limits |

See [Configuration Guide](docs/en_US/configuration.md) for all options.

## Documentation

| Resource | Description |
|----------|-------------|
| [Architecture Overview](docs/en_US/architecture/overview.md) | System architecture and module design |
| [Configuration Guide](docs/en_US/configuration.md) | All configuration options with defaults |
| [Developer Guide](docs/en_US/development/getting-started.md) | Build, test, development workflow |
| [Testing Guide](docs/en_US/development/testing.md) | Unit tests, integration tests, interoperability |
| [Contributing Guide](CONTRIBUTING.md) | How to contribute code |
| [Changelog](CHANGELOG.md) | Release history |
| Crate docs | [oximqtt](./oximqtt/README.md), [oximqtt-bin](./oximqtt-bin/README.md) |

## Ports

| Port | Protocol |
|------|----------|
| 1883 | MQTT (TCP) |
| 8883 | MQTT over TLS |
| 8080 | MQTT over WebSocket |
| 8443 | MQTT over WebSocket-TLS |

## Credits

- From version 0.15, the MQTT codec implementation is partially inspired by and derived from ntex-mqtt.
- Versions 0.13 and earlier relied on maintained forked versions of the ntex and ntex-mqtt crates.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE) at your option.
