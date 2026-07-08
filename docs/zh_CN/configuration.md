# 配置参考

OXIMQTT 使用单一配置文件（`oximqtt.toml`），包含以下配置节。

## 配置加载顺序

后加载的配置覆盖先加载的：

1. `/etc/oximqtt/oximqtt.{toml,json,...}`（可选）
2. `/etc/oximqtt.{toml,json,...}`（可选）
3. `./oximqtt.{toml,json,...}`（可选）
4. `-f` / `--config` CLI 参数（可选）
5. `OXIMQTT_*` 环境变量

## 命令行参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `-f`, `--config` | `String` | 配置文件路径 |
| `-V`, `--version` | 标志 | 打印版本信息 |
| `--id` | `u64` | 覆盖节点 ID |

---

## `[task]` — 全局任务执行器

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `exec_workers` | `usize` | `1000` | 并发任务数 |
| `exec_queue_max` | `usize` | `300000` | 队列容量 |

---

## `[node]` — 集群节点

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `id` | `u64` | `0` | 集群节点 ID |
| `cookie` | `String` | `"oximqttsecretcookie"` | 集群认证共享密钥 |

### `[node.busy]` — 繁忙状态检测

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `check_enable` | `bool` | `true` | 启用繁忙状态检测 |
| `update_interval` | duration | `"2s"` | 重新评估间隔 |
| `loadavg` | `f32` | `80.0` | 系统负载阈值（0-100） |
| `cpuloadavg` | `f32` | `90.0` | CPU 负载阈值（0-100） |
| `handshaking` | `isize` | `0` | 并发握手数阈值 |

---

## `[log]` — 日志

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `to` | string | `"console"` | 输出目标：`"off"`、`"console"`、`"file"`、`"both"` |
| `level` | string | `"info"` | 日志级别：`"trace"`、`"debug"`、`"info"`、`"warn"`、`"error"` |
| `dir` | string | `"/var/log/oximqtt"` | 日志文件目录 |
| `file` | string | `"oximqtt.log"` | 日志文件名 |

---

## `[mqtt]` — 协议限制

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `delayed_publish_max` | `usize` | `100000` | 最大延迟发布消息数 |
| `delayed_publish_immediate` | `bool` | `true` | 超限时立即发送（否则丢弃） |
| `max_sessions` | `isize` | `0` | 最大会话数；`0` = 无限制 |

---

## `[listener.*]` — 网络监听器

监听器按协议分类，每个协议节下是命名监听器的映射：

```toml
[listener.tcp.external]
enable = true
addr = "0.0.0.0:1883"

[listener.tls.internal]
enable = true
addr = "0.0.0.0:8883"
cert = "/path/to/cert.pem"
key = "/path/to/key.pem"
```

支持的协议节：`listener.tcp`、`listener.tls`、`listener.ws`、`listener.wss`、`listener.quic`。

如果未配置 TCP/TLS 监听器，将自动创建 `0.0.0.0:1883` 的默认 TCP 监听器。

### 通用监听器字段

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `enable` | `bool` | `true` | 是否启用 |
| `addr` | string | `"0.0.0.0:1883"` | 绑定地址和端口 |
| `max_connections` | `usize` | `1024000` | 最大并发连接数 |
| `max_handshaking_limit` | `usize` | `500` | 最大并发握手数 |
| `max_packet_size` | bytesize | `"1MB"` | 最大 MQTT 包大小；`0` = 无限制 |
| `backlog` | `i32` | `1024` | TCP 监听队列长度 |
| `nodelay` | `bool` | `false` | TCP_NODELAY |
| `reuseaddr` | `bool` | `true` | SO_REUSEADDR |
| `reuseport` | `bool` | — | SO_REUSEPORT |
| `allow_anonymous` | `bool` | `false` | 允许未认证连接 |
| `min_keepalive` | `u16` | `0` | 最小保活时间（秒） |
| `max_keepalive` | `u16` | `65535` | 最大保活时间（秒） |
| `allow_zero_keepalive` | `bool` | `true` | 允许 keepalive = 0 |
| `keepalive_backoff` | `f32` | `0.75` | 保活回退乘数（> 0.5） |
| `max_inflight` | `u16` | `16` | 最大飞行窗口 QoS 1/2 消息数 |
| `handshake_timeout` | duration | `"15s"` | 握手超时 |
| `max_mqueue_len` | `usize` | `1000` | 每客户端最大消息队列长度 |
| `mqueue_rate_limit` | string | `"4294967295,1s"` | 队列弹出速率：`"<数量>,<时长>"` |
| `max_clientid_len` | `usize` | `65535` | 最大客户端 ID 长度 |
| `max_qos_allowed` | `0/1/2` | `2` | 最大 QoS 级别 |
| `max_topic_levels` | `usize` | `0` | 最大主题层级；`0` = 无限制 |
| `session_expiry_interval` | duration | `"2h"` | 默认会话过期时间 |
| `max_session_expiry_interval` | duration | `"0s"` | 客户端请求的最大过期上限 |
| `message_retry_interval` | duration | `"30s"` | QoS 1/2 重发间隔；`0` = 不重发 |
| `message_expiry_interval` | duration | `"5m"` | 消息 TTL；`0` = 不过期 |
| `max_subscriptions` | `usize` | `0` | 每客户端最大订阅数；`0` = 无限制 |
| `max_topic_aliases` | `u16` | `0` | 最大主题别名数（MQTT 5.0）；`0` = 禁用 |
| `limit_subscription` | `bool` | `false` | 启用订阅限制 |
| `delayed_publish` | `bool` | `false` | 启用延迟发布 |
| `proxy_protocol` | `bool` | `false` | 启用 PROXY 协议 |
| `proxy_protocol_timeout` | duration | `"5s"` | PROXY 协议头超时 |

### TLS 字段（tls、wss、quic 监听器）

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `cert` | string | — | 服务端 TLS 证书路径（PEM） |
| `key` | string | — | 服务端 TLS 私钥路径（PEM） |
| `client_ca_certs` | string | — | 客户端 CA 证书（双向 TLS） |
| `cross_certificate` | `bool` | `false` | 启用双向 TLS |
| `cert_cn_as_username` | `bool` | `false` | 从证书 CN 提取用户名 |
| `cert_subject_dn_as_username` | `bool` | `false` | 从证书 Subject DN 提取用户名 |
| `collect_cert_info` | `bool` | `false` | 收集证书信息到元数据 |
| `idle_timeout` | duration | `"90s"` | QUIC 最大空闲超时 |

---

## `[acl]` — 访问控制

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `disconnect_if_pub_rejected` | `bool` | `true` | 发布被拒绝时断开连接 |
| `rules` | array | `[]` | ACL 规则列表 |

规则格式：`["allow"|"deny", {user="..."} | "all", "pubsub"|"publish"|"subscribe", ["topic/pattern/#"]]`

---

## `[retainer]` — 保留消息

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `max_retained_messages` | `usize` | `0` | 最大保留消息数；`0` = 无限制 |
| `max_payload_size` | bytesize | `"1MB"` | 最大载荷大小 |
| `retained_message_ttl` | duration | `"0m"` | TTL；`0` = 不过期 |

---

## `[auth_jwt]` — JWT 认证

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `disconnect_if_pub_rejected` | `bool` | `true` | 发布被拒绝时断开连接 |
| `disconnect_if_expiry` | `bool` | `false` | JWT 过期时断开连接 |
| `from` | string | `"password"` | JWT 提取来源 |
| `encrypt` | string | `"hmac-based"` | 加密方式 |
| `hmac_secret` | string | `"oximqttsecret"` | HMAC 密钥 |
| `hmac_base64` | `bool` | `false` | 密钥是否为 base64 编码 |
| `validate_claims.exp` | `bool` | `true` | 验证过期声明 |

---

## `[sys_topic]` — 系统主题

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `publish_qos` | `0/1/2` | `1` | $SYS 消息 QoS |
| `publish_interval` | duration | `"1m"` | 发布间隔 |
| `message_expiry_interval` | duration | `"5m"` | 消息 TTL |

---

## 值格式

### Duration（时长）

带单位后缀的字符串：`"30s"`（秒）、`"5m"`（分）、`"2h"`（时）。`0` 或 `"0s"` 表示禁用超时。

### Bytesize（字节大小）

带单位的字符串：`"1MB"`、`"512KB"`、`"0"` 表示无限制。
