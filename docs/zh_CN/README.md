[English](../en_US/README.md) | [**简体中文**](README.md)

# OXIMQTT 文档

欢迎使用 OXIMQTT 文档。本索引提供所有文档资源的结构化概览。

## 快速链接

| 资源 | 说明 |
|----------|-------------|
| [GitHub 仓库](https://github.com/zeaphoo/oximqtt) | 源代码、Issue、讨论 |
| [crates.io](https://crates.io/crates/oximqtt) | 已发布的 crate 版本 |
| [docs.rs](https://docs.rs/oximqtt/latest/oximqtt/) | API 参考（库模式） |

---

## 架构

| 文档 | 说明 |
|----------|-------------|
| [架构概览](architecture/overview.md) | 系统架构、crate 分层、核心模块、会话生命周期 |
| [内置模块](../architecture/overview.md#内置模块) | ACL、JWT 认证、保留消息、系统主题作为核心模块 |
| [钩子系统](../architecture/overview.md#钩子系统) | 23 种钩子类型、Handler 注册、优先级 |

---

## 入门指南

| 文档 | 说明 |
|----------|-------------|
| [安装指南](install.md) | 通过 Docker、二进制包或源码安装 |
| [MQTT 协议支持](mqtt-protocol.md) | 支持的 MQTT 版本、特性和配置 |

---

## 配置

| 文档 | 说明 |
|----------|-------------|
| [配置参考](https://github.com/zeaphoo/oximqtt/blob/master/oximqtt.toml) | 完整配置文件示例 |
| [权限列表](perm-list.md) | 可用权限及其含义 |

---

## 功能

### 认证与访问控制

| 文档 | 说明 |
|----------|-------------|
| [ACL（访问控制列表）](acl.md) | 基于文件的 ACL 规则引擎 |
| [HTTP 认证](auth-http.md) | 外部 HTTP API 认证 |
| [JWT 认证](auth-jwt.md) | JSON Web Token 验证 |

### 消息存储与投递

| 文档 | 说明 |
|----------|-------------|
| [保留消息](retainer.md) | 持久化保留消息存储 |
| [离线消息](offline-message.md) | 断线客户端消息存储 |
| [会话存储](store-session.md) | 会话状态持久化 |
| [消息存储](store-message.md) | 未过期消息持久化 |

### 集群

| 文档 | 说明 |
|----------|-------------|
| [Raft 集群](cluster-raft.md) | 基于 Raft 共识的强一致性集群 |
| [基准测试](benchmark-testing.md) | 性能基准测试报告（100万客户端，15万 msg/s） |

### 桥接

| 文档 | 方向 |
|----------|-----------|
| [MQTT 桥接 - 入站](bridge-ingress-mqtt.md) | 远程 MQTT → 本地 |
| [MQTT 桥接 - 出站](bridge-egress-mqtt.md) | 本地 → 远程 MQTT |
| [Kafka 桥接 - 入站](bridge-ingress-kafka.md) | Kafka → 本地 |
| [Kafka 桥接 - 出站](bridge-egress-kafka.md) | 本地 → Kafka |
| [Pulsar 桥接 - 入站](bridge-ingress-pulsar.md) | Pulsar → 本地 |
| [Pulsar 桥接 - 出站](bridge-egress-pulsar.md) | 本地 → Pulsar |
| [NATS 桥接 - 入站](bridge-ingress-nats.md) | NATS → 本地 |
| [NATS 桥接 - 出站](bridge-egress-nats.md) | 本地 → NATS |
| [ReductStore 桥接 - 出站](bridge-egress-reductstore.md) | 本地 → ReductStore |
| [桥接来源](bridge-origin.md) | 桥接客户端识别 |

### 管理与监控

| 文档 | 说明 |
|----------|-------------|
| [HTTP API](http-api.md) | RESTful 管理 API 参考 |
| [WebHook](web-hook.md) | HTTP 事件通知 |
| [系统主题](sys-topic.md) | `$SYS/` 监控指标 |

### 主题功能

| 文档 | 说明 |
|----------|-------------|
| [主题重写](topic-rewrite.md) | 主题过滤器和名称重写 |
| [自动订阅](auto-subscription.md) | 连接时自动订阅 |
| [共享订阅](shared-subscription.md) | 负载均衡的消费者组 |
| [P2P 消息](p2p-messaging.md) | 客户端间直接消息投递 |

---

## 子项目文档

每个 crate 都有独立的中英文 README：

| Crate | 说明 | README |
|-------|------|--------|
| `oximqtt` | 核心 Broker 库 | [README](../oximqtt/README-CN.md) |
| `oximqttd` | 二进制入口 | [README](../oximqtt-bin/README-CN.md) |
| `oximqtt-codec` | MQTT 协议编解码 | [README](../oximqtt-codec/README-CN.md) |
| `oximqtt-net` | 网络层 | [README](../oximqtt-net/README-CN.md) |
| `oximqtt-conf` | 配置管理 | [README](../oximqtt-conf/README-CN.md) |
| `oximqtt-utils` | 共享工具 | [README](../oximqtt-utils/README-CN.md) |
| `oximqtt-macros` | 过程宏 | [README](../oximqtt-macros/README-CN.md) |
| `oximqtt-test` | 测试框架 | [README](../oximqtt-test/README-CN.md) |

---

## 开发

| 资源 | 说明 |
|------|------|
| [贡献指南](../CONTRIBUTING-CN.md) | 贡献者指导 |
| [更新日志](../CHANGELOG.md) | 版本历史 |
| [开发入门](development/getting-started.md) | 环境搭建、构建、工作流 |
| [测试指南](development/testing.md) | 测试层次、运行测试、编写测试 |
| [测试报告](testing-report.md) | 互操作性测试结果和基准数据 |
| [问題与讨论](https://github.com/zeaphoo/oximqtt/issues) | Issues 和讨论 |

---

## 参考

| 资源 | 说明 |
|------|------|
| [HTTP API 参考](reference/http-api.md) | 完整 REST API 端点参考（36 个端点） |

## 许可证

OXIMQTT 基于 [MIT](https://opensource.org/licenses/MIT) 或 [Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0) 许可证。
