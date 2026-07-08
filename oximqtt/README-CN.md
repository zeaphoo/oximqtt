[English](README.md) | [**简体中文**](README-CN.md)

# OXIMQTT-Server

[![crates.io page](https://img.shields.io/crates/v/oximqtt.svg)](https://crates.io/crates/oximqtt)
[![docs.rs page](https://docs.rs/oximqtt/badge.svg)](https://docs.rs/oximqtt/latest/oximqtt/)

核心 MQTT Broker 库 — 会话管理、路由、Hook、内置模块和指标。

## 模块结构

```
oximqtt/src/
├── lib.rs          — 重新导出：oximqtt_codec as codec、oximqtt_net as net、oximqtt_utils as utils
│
├── acl.rs          — ACL 类型（ACLConfig、AclCheckFn、AuthInfo）
├── args.rs         — CommandArgs 结构体（node_id）
├── builtins/       — [feature: full] 内置模块（acl、auth_jwt、retainer、sys_topic）
├── context.rs      — ServerContext Builder（流畅 API：.node()、.task_exec_workers() 等）
├── executor.rs     — 异步任务执行器（封装 rust-box task-exec-queue）
├── extend.rs       — 扩展点（10 个 RwLock 保护的插槽）
├── fitter.rs       — 主题过滤器匹配引擎
├── hook.rs         — Hook 系统（Hook trait，10+ 个 Hook 点：message_publish、client_keepalive、session_created 等）
├── inflight.rs     — 进行中消息追踪（InInflight、OutInflight、OutInflightMessage）
├── node.rs         — 节点管理（Node::new()、Node::version()）
├── queue.rs        — 消息队列（Limiter、Policy、速率限制队列）
├── router.rs       — 基于主题的消息路由（publish、subscribe、unsubscribe、离线投递）
├── server.rs       — MqttServer（Builder：.listener()、.build()、.start()；连接接受循环）
├── session.rs      — 会话处理（~2400 行：连接、断开、订阅、发布、QoS 流程）
├── shared.rs       — 共享订阅（$share/{group}/{topic}）
├── topic.rs        — 主题解析/验证（TopicFilter、parse_topic_filter、topic_size）
├── trie.rs         — 订阅匹配的 Trie 树
├── types.rs        — 核心类型（~3000 行：ConnectInfo、Publish、Packet、Reason、Id、SessionTx 等）
├── v3.rs           — MQTT v3.1.1 协议处理器
├── v5.rs           — MQTT v5.0 协议处理器
│
├── delayed.rs      — 延迟消息发布
├── message.rs      — 消息存储子系统
├── metrics.rs      — 指标收集
├── retain.rs       — 保留消息存储
├── stats.rs        — 运行时统计
├── subscribe.rs    — 订阅服务
```

## Feature 标志

| Feature | 启用的依赖 | 说明 |
|---------|-------------|------|
| `default` | tls, ws, quic | 所有传输层 |
| `tls` | oximqtt-net/tls | TLS 传输 |
| `ws` | oximqtt-net/ws | WebSocket 传输 |
| `quic` | oximqtt-net/quic | QUIC 传输 |

其他所有功能（延迟发布、保留消息、指标统计、共享订阅、自动订阅等）均作为内置模块无条件编译。

## 重新导出

```rust
pub use oximqtt_codec as codec;   // MQTT 协议编解码
pub use oximqtt_net as net;       // 网络层（Builder、MqttStream 等）
pub use oximqtt_utils as utils;   // 工具（Bytesize、NodeAddr 等）
pub use oximqtt_macros as macros; // [feature: metrics] 派生宏
pub use net::{Error, Result};   // 重新导出的错误类型
```

## 使用方式

```rust,no_run
use oximqtt::context::ServerContext;
use oximqtt::net::Builder;
use oximqtt::server::MqttServer;
use oximqtt::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let scx = ServerContext::new().build().await;

    // 初始化内置模块（需要 "full" feature）
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

## 示例

参见 `oximqtt/examples/`：`simple`、`simple_tls`、`simple_ws`、`simple_wss`、`simple_quic`、`multi`、`plugin`、`simple_quic_client`。

## 许可证

MIT OR Apache-2.0
