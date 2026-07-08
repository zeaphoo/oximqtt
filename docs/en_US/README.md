[**English**](README.md) | [简体中文](../zh_CN/README.md)

# OXIMQTT Documentation

Welcome to the OXIMQTT documentation. This index provides a structured overview of all available documentation resources.

## Quick Links

| Resource | Description |
|----------|-------------|
| [GitHub Repository](https://github.com/zeaphoo/oximqtt) | Source code, issues, discussions |
| [crates.io](https://crates.io/crates/oximqtt) | Published crate versions |
| [docs.rs](https://docs.rs/oximqtt/latest/oximqtt/) | API reference (library mode) |

---

## Architecture

| Document | Description |
|----------|-------------|
| [Architecture Overview](architecture/overview.md) | System architecture, core modules, session lifecycle |
| [Built-in Modules](architecture/overview.md#built-in-modules) | ACL, JWT auth, retainer, sys-topic as core modules |
| [Hook System](architecture/overview.md#hook-system) | All 23 hook types, handler registration, priority |
| [Message Flow](architecture/overview.md#message-flow) | End-to-end publish/subscribe flow with diagrams |

---

## Getting Started

| Document | Description |
|----------|-------------|
| [Installation Guide](install.md) | Install via binary package or source build |
| [MQTT Protocol Support](mqtt-protocol.md) | Supported MQTT versions, features, and configuration |

---

## Configuration

| Document | Description |
|----------|-------------|
| [Configuration Reference](configuration.md) | All configuration options with defaults |
| [Configuration File Example](https://github.com/zeaphoo/oximqtt/blob/master/oximqtt.toml) | Full configuration file example |
| [Permission List](perm-list.md) | Available permissions and their meanings |

---

## Built-in Modules

| Document | Description |
|----------|-------------|
| [ACL (Access Control List)](acl.md) | File-based ACL rule engine |
| [JWT Authentication](auth-jwt.md) | JSON Web Token validation |
| [Retained Messages](retainer.md) | Persistent retained message storage |
| [System Topics](sys-topic.md) | `$SYS/` monitoring metrics |

---

## Benchmarking & Testing

| Document | Description |
|----------|-------------|
| [Test Report](testing-report.md) | Interoperability results and benchmark data |

---

## Crate Documentation

| Crate | Description | README |
|-------|-------------|--------|
| `oximqtt` | Core broker library (codec, net, utils, conf, builtins) | [README](../../oximqtt/README.md) |
| `oximqttd` | Binary entry point | [README](../../oximqtt-bin/README.md) |
| `oximqtt-test` | Test harness | [README](../../oximqtt-test/README.md) |

---

## Development

| Resource | Description |
|----------|-------------|
| [Contributing Guide](../../CONTRIBUTING.md) | Contribution guidelines |
| [Changelog](../../CHANGELOG.md) | Release history |
| [Developer Getting Started](development/getting-started.md) | Dev environment setup, build, workflow |
| [Testing Guide](development/testing.md) | Test layers, running tests, writing tests |
| [FAQ](https://github.com/zeaphoo/oximqtt/issues) | Issues and discussions |

---

## License

OXIMQTT is licensed under [MIT](https://opensource.org/licenses/MIT) or [Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0) at your option.
