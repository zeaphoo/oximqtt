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
| [架构概览](architecture/overview.md) | 系统架构、核心模块、会话生命周期 |
| [内置模块](architecture/overview.md#内置模块) | ACL、JWT 认证、保留消息、系统主题作为核心模块 |
| [钩子系统](architecture/overview.md#钩子系统) | 23 种钩子类型、Handler 注册、优先级 |

---

## 入门指南

| 文档 | 说明 |
|----------|-------------|
| [安装指南](install.md) | 通过二进制包或源码安装 |
| [MQTT 协议支持](mqtt-protocol.md) | 支持的 MQTT 版本、特性和配置 |

---

## 配置

| 文档 | 说明 |
|----------|-------------|
| [配置参考](configuration.md) | 所有配置项及默认值 |
| [配置文件示例](https://github.com/zeaphoo/oximqtt/blob/master/oximqtt.toml) | 完整配置文件示例 |
| [权限列表](perm-list.md) | 可用权限及其含义 |

---

## 内置模块

| 文档 | 说明 |
|----------|-------------|
| [ACL（访问控制列表）](acl.md) | 基于文件的 ACL 规则引擎 |
| [JWT 认证](auth-jwt.md) | JSON Web Token 验证 |
| [保留消息](retainer.md) | 持久化保留消息存储 |
| [系统主题](sys-topic.md) | `$SYS/` 监控指标 |

---

## 基准测试与测试

| 文档 | 说明 |
|----------|-------------|
| [测试报告](testing-report.md) | 互操作性测试结果和基准数据 |

---

## 项目文档

| Crate | 说明 | README |
|-------|------|--------|
| `oximqtt` | 核心 Broker 库（codec、net、utils、conf、builtins） | [README](../../oximqtt/README-CN.md) |
| `oximqttd` | 二进制入口 | [README](../../oximqtt-bin/README-CN.md) |
| `oximqtt-test` | 测试框架 | [README](../../oximqtt-test/README-CN.md) |

---

## 开发

| 资源 | 说明 |
|------|------|
| [贡献指南](../../CONTRIBUTING-CN.md) | 贡献者指导 |
| [更新日志](../../CHANGELOG.md) | 版本历史 |
| [开发入门](development/getting-started.md) | 环境搭建、构建、工作流 |
| [测试指南](development/testing.md) | 测试层次、运行测试、编写测试 |
| [问题与讨论](https://github.com/zeaphoo/oximqtt/issues) | Issues 和讨论 |

---

## 许可证

OXIMQTT 基于 [Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0) 许可证。
