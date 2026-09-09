[English](../en_US/testing-report.md) | [**简体中文**](testing-report.md)

# OXIMQTT 测试报告

本文档提供 OXIMQTT MQTT Broker 的详细测试结果，包括针对 paho.mqtt.testing 套件的互操作性测试和性能基准数据。

---

## 互操作性测试

OXIMQTT 通过了官方 [paho.mqtt.testing](https://github.com/eclipse/paho.mqtt.testing) 互操作性测试套件。

### 环境准备

```bash
git clone https://github.com/eclipse/paho.mqtt.testing.git
cd paho.mqtt.testing/interoperability

# 另开终端启动 OXIMQTT Broker
./target/release/oximqttd
```

### MQTT V3.1.1 — 11/11 通过

| 测试 | 结果 | 备注 |
|------|------|------|
| `test_retained_messages` | ✅ 通过 | — |
| `test_zero_length_clientid` | ✅ 通过 | — |
| `will_message_test` | ✅ 通过 | — |
| `test_offline_message_queueing` | ✅ 通过 | — |
| `test_overlapping_subscriptions` | ✅ 通过 | — |
| `test_keepalive` | ✅ 通过 | — |
| `test_redelivery_on_reconnect` | ✅ 通过 | — |
| `test_dollar_topics` | ✅ 通过 | — |
| `test_unsubscribe` | ✅ 通过 | — |
| `test_subscribe_failure` | ✅ 通过 | 需在 `acl` 配置首行添加：`["deny", "all", "subscribe", ["test/nosubscribe"]]` |
| `test_zero_length_clientid` | ✅ 通过 | — |

### MQTT V5.0 — 24/24 通过

| 测试 | 结果 |
|------|------|
| `test_retained_message` | ✅ 通过 |
| `test_will_message` | ✅ 通过 |
| `test_offline_message_queueing` | ✅ 通过 |
| `test_dollar_topics` | ✅ 通过 |
| `test_unsubscribe` | ✅ 通过 |
| `test_session_expiry` | ✅ 通过 |
| `test_basic` | ✅ 通过 |
| `test_overlapping_subscriptions` | ✅ 通过 |
| `test_redelivery_on_reconnect` | ✅ 通过 |
| `test_payload_format` | ✅ 通过 |
| `test_publication_expiry` | ✅ 通过 |
| `test_subscribe_options` | ✅ 通过 |
| `test_assigned_clientid` | ✅ 通过 |
| `test_subscribe_identifiers` | ✅ 通过 |
| `test_request_response` | ✅ 通过 |
| `test_server_topic_alias` | ✅ 通过 |
| `test_client_topic_alias` | ✅ 通过 |
| `test_maximum_packet_size` | ✅ 通过 |
| `test_keepalive` | ✅ 通过 |
| `test_zero_length_clientid` | ✅ 通过 |
| `test_user_properties` | ✅ 通过 |
| `test_flow_control2` | ✅ 通过 |
| `test_flow_control1` | ✅ 通过 |
| `test_will_delay` | ✅ 通过 |
| `test_server_keep_alive` | ✅ 通过（需将 `oximqtt.toml` 中 `max_keepalive` 改为 60） |
| `test_subscribe_failure` | ✅ 通过（ACL 配置同 v3.1.1） |

---

## 集成测试框架

`oximqtt-test` crate 提供了自定义测试框架，在 paho 之外还包含以下套件：

| 套件 | 用例数 | 说明 |
|-------|--------|------|
| `functional_v3` | 2 | MQTT 3.1 基本操作 |
| `functional_v311` | 10 | MQTT 3.1.1 协议合规 |
| `functional_v5` | 5 | MQTT 5.0 协议合规 |
| `stress` | 3 | 连接负载、发布 QPS、扇出测试 |
| `chaos` | 6 | Broker 重启、连接风暴、重连、QoS 1 可靠性、慢消费者 |

```bash
# 运行所有测试套件
cargo build --release
cargo build -p oximqtt-test --release
./target/release/mqtt_harness --workspace .
```

---

## 性能基准

### 环境

| 项目 | 内容 |
|------|------|
| 操作系统 | x86_64 GNU/Linux, Rocky Linux 9.2 (Blue Onyx) |
| CPU | Intel(R) Xeon(R) CPU E5-2696 v3 @ 2.30GHz, 72 线程 |
| 内存 | DDR3/2333, 128 GB |
| 磁盘 | 2 TB |
| 容器 | Podman v4.4.1 |
| 测试工具 | `rmqtt/rmqtt-bench:latest` (v0.1.3) |
| MQTT Broker | `zeaphoo/oximqtt:latest` (v0.22.0) |

*测试客户端和 Broker 同机部署。*

### 连接并发性能

| 指标 | 数值 |
|------|------|
| 并发客户端总数 | 1,000,000 |
| 连接握手速率 | 5,500-7,000/秒 |

### 消息吞吐性能

| 指标 | 数值 |
|------|------|
| 订阅客户端数 | 1,000,000 |
| 发布客户端数 | 40 |
| 消息吞吐速率 | 150,000 条/秒 |

### 仓库内置 `mqtt-bench` 验证结果

`oximqtt-bench`（二进制 `mqtt-bench`）是独立压测工具，自带 MQTT
3.1.1 / 5.0 协议编解码器，**不复用 broker 自身 codec**，相当于以外部客户端的
视角去压测 broker。单机验证（客户端与 broker 同机、回环网卡、256 B 消息、
broker `nodelay = true`）针对**当前仓库代码**（含 QoS 2 两项修复：规范 PUBREL
reason `0x02` 与入向流控），全部使用 `mqtt-bench` 默认参数，结果：

| 场景 | 结果 |
|------|------|
| 连接数（v3.1.1，`-c 20000`） | 20,000/20,000 成功 |
| 订阅数（`-c 10000 -S`） | 10,000/10,000 成功 |
| QoS 0 入口，200 发布端打满 | 约 145 万条/秒（约 3.1 Gbps） |
| QoS 1 入口，200 发布端打满 | 约 39.9 万条/秒，全部确认无丢失 |
| QoS 2 入口，200 发布端（默认参数） | 约 21.8 万条/秒，0 断连 |
| QoS 1 入口，MQTT 5.0 | 约 37 万条/秒 |
| QoS 2 入口，MQTT 5.0（reason 0x02） | 约 20.4 万条/秒，0 断连 |
| QoS 1 端到端 1:1，100 连接，节奏发送 | 600,672 发 = 收，p50 ≈ 0.9 ms |
| QoS 2 端到端 1:1，30 连接，v5 节奏发送 | 36,163 发 = 收，p50 ≈ 0.3 ms |
| 抖动模式 `-T`，500 连接，`-D 0.2` | 重连期间仍保持约 9.1 万条/秒 |

独立编解码器暴露出的互通性问题（均经逐字节探针确认）——**均已在当前仓库修复**：

1. **MQTT 5.0 QoS 2 PUBREL reason code**：原 codec 枚举只有 `Success = 0` 与
   `PacketIdNotFound = 146`，把规范唯一允许的 `0x02`（Send Onward，§3.4.4.1）
   当作畸形包并断连；同源客户端因此从未触发。现已在 `PublishAck2Reason`
   中补上 `SendOnward = 2`，broker 发出的 PUBREL 也使用 `0x02`。
2. **QoS 2 在途窗口上限**：连接级 `listener.*.max_inflight`（默认 16）原先超限
   即断连（v3.1.1 与 v5 均是）。现改为入向流控：超窗消息暂缓（推迟 PUBREC，
   待 PUBREL 释放窗口后补发），不再掐连接。压测工具会自动遵守 v5 Receive
   Maximum 公告。

---

## 许可证

Apache-2.0
