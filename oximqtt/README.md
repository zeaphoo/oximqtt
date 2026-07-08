[**English**](README.md) | [简体中文](README-CN.md)

# OXIMQTT-Server

[![crates.io page](https://img.shields.io/crates/v/oximqtt.svg)](https://crates.io/crates/oximqtt)
[![docs.rs page](https://docs.rs/oximqtt/badge.svg)](https://docs.rs/oximqtt/latest/oximqtt/)

Core MQTT broker library — session management, routing, hooks, built-in modules, and metrics.

## Module structure

```
oximqtt/src/
├── lib.rs          — re-exports: oximqtt_codec as codec, oximqtt_net as net, oximqtt_utils as utils
│
├── acl.rs          — Access Control List types (ACLConfig, AclCheckFn, AuthInfo)
├── args.rs         — CommandArgs struct (node_id)
├── builtins/       — [feature: full] Built-in modules (acl, auth_jwt, retainer, sys_topic)
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
├── shared.rs       — Shared subscriptions ($share/{group}/{topic})
├── topic.rs        — Topic parsing/validation (TopicFilter, parse_topic_filter, topic_size)
├── trie.rs         — Topic trie for subscription matching
├── types.rs        — Core types (~3000 lines: ConnectInfo, Publish, Packet, Reason, Id, SessionTx, etc.)
├── v3.rs           — MQTT v3.1.1 protocol handler
├── v5.rs           — MQTT v5.0 protocol handler
│
├── delayed.rs      — [feature: delayed] Delayed message publishing
├── message.rs      — [feature: msgstore] Message storage subsystem
├── metrics.rs      — [feature: metrics] Metrics collection
├── retain.rs       — [feature: retain] Retained message storage
├── stats.rs        — [feature: stats] Runtime statistics
├── subscribe.rs    — [feature: auto-subscription|shared-subscription] Subscription services
```

## Feature flags

| Feature | Deps enabled | What it enables |
|---------|-------------|-----------------|
| `metrics` | oximqtt-macros/metrics | Metrics collection |
| `stats` | — | Runtime statistics tracking |
| `tls` | oximqtt-net/tls | TLS transport |
| `ws` | oximqtt-net/ws | WebSocket transport |
| `quic` | oximqtt-net/quic | QUIC transport |
| `delayed` | — | Delayed message publishing |
| `retain` | — | Retained message storage |
| `msgstore` | — | Message persistence |
| `shared-subscription` | — | Shared subscriptions ($share/) |
| `auto-subscription` | — | Auto-subscribe on connect |
| `limit-subscription` | — | Subscription limiting |
| `macros` | dep:oximqtt-macros, metrics | Derive macros |
| `full` | All of the above + builtins | All features |
| `debug` | — | Debug mode |
| `default` | (none) | Minimal build |

## Re-exports

```rust
pub use oximqtt_codec as codec;   // MQTT protocol codec
pub use oximqtt_net as net;       // Network layer (Builder, MqttStream, etc.)
pub use oximqtt_utils as utils;   // Utilities (Bytesize, NodeAddr, etc.)
pub use oximqtt_macros as macros; // [feature: metrics] Derive macros
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

MIT OR Apache-2.0
