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
| [Architecture Overview](architecture/overview.md) | System architecture, crate layers, core modules, session lifecycle |
| [Built-in Modules](../architecture/overview.md#built-in-modules) | ACL, JWT auth, retainer, sys-topic as core modules |
| [Hook System](../architecture/overview.md#hook-system) | All 23 hook types, handler registration, priority |
| [Message Flow](../architecture/overview.md#message-flow) | End-to-end publish/subscribe flow with diagrams |

---

## Getting Started

| Document | Description |
|----------|-------------|
| [Installation Guide](install.md) | Install via Docker, binary package, or source build |
| [MQTT Protocol Support](mqtt-protocol.md) | Supported MQTT versions, features, and configuration |

---

## Configuration

| Document | Description |
|----------|-------------|
| [Configuration Reference](https://github.com/zeaphoo/oximqtt/blob/master/oximqtt.toml) | Full configuration file example |
| [Permission List](perm-list.md) | Available permissions and their meanings |

---

## Features

### Authentication & Access Control

| Document | Description |
|----------|-------------|
| [ACL (Access Control List)](acl.md) | File-based ACL rule engine |
| [HTTP Authentication](auth-http.md) | External HTTP API authentication |
| [JWT Authentication](auth-jwt.md) | JSON Web Token validation |

### Message Storage & Delivery

| Document | Description |
|----------|-------------|
| [Retained Messages](retainer.md) | Persistent retained message storage |
| [Offline Messages](offline-message.md) | Message storage for disconnected clients |
| [Session Storage](store-session.md) | Session state persistence |
| [Message Storage](store-message.md) | Unexpired message persistence |

### Clustering

| Document | Description |
|----------|-------------|
| [Raft Cluster](cluster-raft.md) | Strongly consistent clustering via Raft consensus |
| [Benchmark Testing](benchmark-testing.md) | Performance benchmarks (1M clients, 150K msg/s) |

### Bridges

| Document | Direction |
|----------|-----------|
| [MQTT Bridge - Ingress](bridge-ingress-mqtt.md) | Remote MQTT → Local |
| [MQTT Bridge - Egress](bridge-egress-mqtt.md) | Local → Remote MQTT |
| [Kafka Bridge - Ingress](bridge-ingress-kafka.md) | Kafka → Local |
| [Kafka Bridge - Egress](bridge-egress-kafka.md) | Local → Kafka |
| [Pulsar Bridge - Ingress](bridge-ingress-pulsar.md) | Pulsar → Local |
| [Pulsar Bridge - Egress](bridge-egress-pulsar.md) | Local → Pulsar |
| [NATS Bridge - Ingress](bridge-ingress-nats.md) | NATS → Local |
| [NATS Bridge - Egress](bridge-egress-nats.md) | Local → NATS |
| [ReductStore Bridge - Egress](bridge-egress-reductstore.md) | Local → ReductStore |
| [Bridge Origin](bridge-origin.md) | Bridge client identification |

### Management & Monitoring

| Document | Description |
|----------|-------------|
| [HTTP API](http-api.md) | RESTful management API reference |
| [WebHook](web-hook.md) | HTTP event notifications |
| [System Topics](sys-topic.md) | `$SYS/` monitoring metrics |

### Topic Features

| Document | Description |
|----------|-------------|
| [Topic Rewrite](topic-rewrite.md) | Topic filter and name rewriting |
| [Auto Subscription](auto-subscription.md) | Automatic subscription on connect |
| [Shared Subscription](shared-subscription.md) | Load-balanced consumer groups |
| [P2P Messaging](p2p-messaging.md) | Direct client-to-client messaging |

---

## Crate Documentation

Each crate has its own bilingual README:

| Crate | Description | README |
|-------|-------------|--------|
| `oximqtt` | Core broker library | [README](../oximqtt/README.md) |
| `oximqttd` | Binary entry point | [README](../oximqtt-bin/README.md) |
| `oximqtt-codec` | MQTT protocol codec | [README](../oximqtt-codec/README.md) |
| `oximqtt-net` | Network layer (TCP/TLS/WS/QUIC) | [README](../oximqtt-net/README.md) |
| `oximqtt-conf` | Configuration management | [README](../oximqtt-conf/README.md) |
| `oximqtt-utils` | Shared utilities | [README](../oximqtt-utils/README.md) |
| `oximqtt-macros` | Procedural macros | [README](../oximqtt-macros/README.md) |
| `oximqtt-test` | Test harness | [README](../oximqtt-test/README.md) |

---

## Development

| Resource | Description |
|----------|-------------|
| [Contributing Guide](../CONTRIBUTING.md) | Contribution guidelines |
| [Changelog](../CHANGELOG.md) | Release history |
| [Developer Getting Started](development/getting-started.md) | Dev environment setup, build, workflow |
| [Testing Guide](development/testing.md) | Test layers, running tests, writing tests |
| [Test Report](testing-report.md) | Interoperability results and benchmark data |
| [FQA](https://github.com/zeaphoo/oximqtt/issues) | Issues and discussions |

---

## Reference

| Resource | Description |
|----------|-------------|
| [HTTP API Reference](reference/http-api.md) | Complete REST API endpoint reference (36 endpoints) |

---

## License

OXIMQTT is licensed under [MIT](https://opensource.org/licenses/MIT) or [Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0) at your option.
