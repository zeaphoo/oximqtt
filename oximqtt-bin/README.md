[**English**](README.md) | [简体中文](README-CN.md)

# oximqttd

[![crates.io page](https://img.shields.io/crates/v/oximqtt.svg)](https://crates.io/crates/oximqtt)
![Rust](https://img.shields.io/badge/rust-1.89%2B-blue)

Official binary entry point for the OXIMQTT MQTT broker.

## What it does

- **Startup flow**: Parse CLI args → initialize `oximqtt::conf::Settings` singleton → install rustls crypto backend → init tracing logger → create `oximqtt::context::ServerContext` → initialize built-in modules (ACL, Retainer, JWT Auth, Sys Topic) → bind configured listeners → start MQTT server
- **Listener types**: TCP, TLS, WebSocket (WS), TLS-WebSocket (WSS), QUIC
- **Signal handling**: `Ctrl+C` on Windows, `SIGTERM` + `SIGINT` on Unix; 100ms graceful delay before exit
- **Logging**: Configured via `oximqtt::conf::logging::Log` — supports `off/console/file/both` modes, UTC+8 timestamps, non-blocking file writer
- **High performance**: Built with Rust for memory safety and speed

## Build

```bash
cargo build -p oximqttd --release
# Artifact: target/release/oximqttd (or oximqttd.exe)
```

## Run

```bash
./target/release/oximqttd
./target/release/oximqttd -f /path/to/oximqtt.toml
./target/release/oximqttd --config /path/to/oximqtt.toml
./target/release/oximqttd --id 1
```

## CLI arguments

Defined by `oximqtt::conf::Options` (via `clap::Parser`):

| Argument | Type | Description |
|----------|------|-------------|
| `-f`, `--config` | `Option<String>` | Config file path |
| `-V`, `--version` | `bool` | Print version info |
| `--id` | `Option<u64>` | Node ID |

## Configuration

Loaded by `oximqtt::conf::Settings` from the following paths (in priority order):

1. `/etc/oximqtt/oximqtt.{toml,json,...}` (optional)
2. `/etc/oximqtt.{toml,json,...}` (optional)
3. `./oximqtt.{toml,json,...}` (optional)
4. `-f` / `--config` specified file (optional)
5. `OXIMQTT_*` environment variables

## Related crates

- [oximqtt] — Core MQTT Broker library

[oximqtt]: https://crates.io/crates/oximqtt

## License

MIT OR Apache-2.0
