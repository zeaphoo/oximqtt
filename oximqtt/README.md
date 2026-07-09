[**English**](README.md) | [简体中文](README-CN.md)

# OXIMQTT-Server

[![crates.io page](https://img.shields.io/crates/v/oximqtt.svg)](https://crates.io/crates/oximqtt)
[![docs.rs page](https://docs.rs/oximqtt/badge.svg)](https://docs.rs/oximqtt/latest/oximqtt/)

Core MQTT broker library — session management, routing, hooks, built-in modules, and metrics.

## Module structure

```
oximqtt/src/
├── lib.rs          — modules: codec, net, utils, conf, builtins, and broker core
│
├── acl.rs          — Access Control List types (ACLConfig, AclCheckFn, AuthInfo)
├── args.rs         — CommandArgs struct (node_id)
├── builtins/       — Built-in modules (acl, auth_jwt, retainer, sys_topic)
├── context.rs      — ServerContext builder (fluent API: .node(), .task_exec_workers(), etc.)
├── executor.rs     — Async task executor (wraps rust-box task-exec-queue)
├── extend.rs       — Extension points with RwLock-protected components (10 slots)
├── fitter.rs       — Topic filter matching engine
├── hook.rs         — Hook system (Hook trait, 10+ hook points: message_publish, client_keepalive, session_created, etc.)
├── inflight.rs     — In-flight message tracking (InInflight, OutInflight, OutInflightMessage)
├── node.rs         — Node management (Node::new(), Node::version(), Node::rustc_version())
├── queue.rs        — Message queue (Limiter, Policy, rate-limited queue)
├── router.rs       — Topic-based message router (publish, subscribe, unsubscribe, route to offline)
├── server.rs       — MqttServer (builder: .listener(), .build(), .start(); accept loop)
├── session.rs      — Session handling (~2400 lines: connect, disconnect, subscribe, publish, QoS flow)
├── topic.rs        — Topic parsing/validation (TopicFilter, parse_topic_filter, topic_size)
├── trie.rs         — Topic trie for subscription matching
├── types.rs        — Core types (~3000 lines: ConnectInfo, Publish, Packet, Reason, Id, SessionTx, etc.)
├── v3.rs           — MQTT v3.1.1 protocol handler
├── v5.rs           — MQTT v5.0 protocol handler
│
├── delayed.rs      — Delayed message publishing
├── message.rs      — Message storage subsystem
├── metrics.rs      — Metrics collection
├── retain.rs       — Retained message storage
├── stats.rs        — Runtime statistics
├── subscribe.rs    — Subscription services
```

## Feature flags

| Feature | Deps enabled | What it enables |
|---------|-------------|-----------------|
| `default` | tls, ws, quic | All transport layers |
| `tls` | rustls, tokio-rustls, x509-parser | TLS transport |
| `ws` | tokio-tungstenite | WebSocket transport |
| `quic` | tls (implies) | QUIC transport |

All other functionality (delayed publish, retained messages, metrics, stats, etc.) is compiled unconditionally as built-in modules.

## Modules

```rust
pub mod codec;   // MQTT protocol codec (v3/v5)
pub mod net;     // Network layer (Builder, MqttStream, etc.)
pub mod utils;   // Utilities (Bytesize, NodeAddr, etc.)
pub mod conf;    // Configuration management
pub use net::{Error, Result};   // Re-exported error types
```

## Usage

```rust,no_run
use oximqtt::context::ServerContext;
use oximqtt::net::Builder;
use oximqtt::server::MqttServer;
use oximqtt::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let scx = ServerContext::new().build().await;

    // Initialize built-in modules (requires "full" feature)
    oximqtt::builtins::init_all(&scx).await?;

    MqttServer::new(scx)
        .listener(Builder::new().name("tcp").laddr("0.0.0.0:1883".parse()?).bind()?.tcp()?)
        .listener(Builder::new().name("ws").laddr("0.0.0.0:8080".parse()?).bind()?.ws()?)
        .build()
        .run()
        .await?;
    Ok(())
}
```

## Examples

See `oximqtt/examples/` for: `simple`, `simple_tls`, `simple_ws`, `simple_wss`, `simple_quic`, `multi`, `plugin`, `simple_quic_client`.

## License

Apache-2.0
