# OXIMQTT Broker

[![GitHub Release](https://img.shields.io/github/release/zeaphoo/oximqtt?color=brightgreen)](https://github.com/zeaphoo/oximqtt/releases)
[![Rust Version](https://img.shields.io/badge/rust-1.89.0%2B-blue)](https://blog.rust-lang.org/2025/08/07/Rust-1.89.0/)
[![crates.io](https://img.shields.io/crates/v/oximqtt.svg)](https://crates.io/crates/oximqtt)
[![docs.rs](https://docs.rs/oximqtt/badge.svg)](https://docs.rs/oximqtt/latest/oximqtt/)

[English](README.md) | [**简体中文**](README-CN.md)

轻量级高性能 MQTT Broker，100% Rust 安全代码。基于 [tokio](https://crates.io/crates/tokio) 构建，适用于 IoT、M2M 和移动应用。

## 功能特色

- 支持 MQTT v3.1、v3.1.1 和 v5.0 协议
- QoS 0、1、2 消息投递
- TLS / WebSocket / WebSocket-TLS / QUIC 传输
- 内置模块（在 `oximqtt.toml` 中配置）：
  - **ACL** — 基于规则的发布/订阅授权
  - **Retainer** — 保留消息存储
  - **JWT 认证** — 基于 JWT 的客户端认证
  - **系统主题** — `$SYS` 系统主题发布
- 排它订阅和限制订阅
- 延迟发布（`$delayed/{Interval}/{Topic}`）
- 指标监控和运行时统计
- 速率限制、飞行窗口和消息队列
- 保留消息和遗嘱消息
- 离线消息队列
- 库模式 — 作为 Rust crate 嵌入使用

## 快速开始

### 从源码编译

```bash
cargo build -p oximqttd --release
./target/release/oximqttd
```

### 作为库使用

```toml
[dependencies]
oximqtt = "0.22"
```

详见 [oximqtt crate 文档](./oximqtt/README-CN.md)。

## 配置

所有配置集中在 `oximqtt.toml` 文件中，主要配置节：

| 配置节 | 说明 |
|--------|------|
| `[listener.*]` | TCP、TLS、WebSocket、WSS、QUIC 监听器 |
| `[acl]` | 访问控制规则 |
| `[retainer]` | 保留消息存储 |
| `[auth_jwt]` | JWT 认证 |
| `[sys_topic]` | 系统主题发布 |
| `[node]` | 节点标识和集群设置 |
| `[log]` | 日志配置 |
| `[mqtt]` | 协议限制 |

详见 [配置指南](docs/zh_CN/configuration.md)。

## 文档资源

| 资源 | 说明 |
|------|------|
| [架构概览](docs/zh_CN/architecture/overview.md) | 系统架构和模块设计 |
| [配置指南](docs/zh_CN/configuration.md) | 所有配置项及默认值 |
| [开发入门](docs/zh_CN/development/getting-started.md) | 构建、测试、开发工作流 |
| [测试指南](docs/zh_CN/development/testing.md) | 单元测试、集成测试、互操作性 |
| [贡献指南](CONTRIBUTING-CN.md) | 如何贡献代码 |
| [更新日志](CHANGELOG.md) | 版本历史 |
| 项目文档 | [oximqtt](./oximqtt/README-CN.md)、[oximqtt-bin](./oximqtt-bin/README-CN.md) |

## 端口

| 端口 | 协议 |
|------|------|
| 1883 | MQTT (TCP) |
| 8883 | MQTT over TLS |
| 8080 | MQTT over WebSocket |
| 8443 | MQTT over WebSocket-TLS |

以上为默认端口，可在 `oximqtt.toml` 的 `[listener.*]` 配置段中修改。详见[配置指南](docs/zh_CN/configuration.md#listener--网络监听器)。

## Credits

- 从 0.15 版本开始，本项目的 MQTT 编解码实现部分参考并借鉴了 ntex-mqtt 的实现。
- 在 0.13 及之前的版本，本项目依赖了维护的 ntex 和 ntex-mqtt fork 版本作为依赖库。

## 许可证

基于 [MIT](LICENSE-MIT) 或 [Apache 2.0](LICENSE-APACHE) 许可证。
