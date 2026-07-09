[English](../en_US/retainer.md)  | 简体中文

# 保留消息

客户端发布消息时设置了**retain**标记，消息将被保留。然后当客户端订阅此消息匹配的主题过滤器时，将收到此保留消息。

**OXIMQTT 0.4.0**及之后版本默认将关闭**保留消息**功能。开始**保留消息**功能需要启用**retainer**内置模块和**listener.tcp.\<xxxx\>.retain_available**配置项。

注意：**OXIMQTT 0.11.0**及之后版本已经移除：**listener.tcp.\<xxxx\>.retain_available**配置项

#### 内置模块：

```bash
retainer
```

#### 配置位置：

```bash
oximqtt.toml 中的 [retainer] 配置段
```

#### 配置项：

```bash
##--------------------------------------------------------------------
## [retainer] 配置段 (oximqtt.toml)
##--------------------------------------------------------------------

# The maximum number of retained messages, where 0 indicates no limit. After the number of reserved messages exceeds
# the maximum limit, existing reserved messages can be replaced, but reserved messages cannot be stored for new topics.
retainer.max_retained_messages = 0

# The maximum Payload value for retaining messages. After the Payload size exceeds the maximum value, the OXIMQTT
# message server will process the received reserved message as a regular message.
retainer.max_payload_size = "1MB"

# TTL for retained messages. Set to 0 for no expiration.
# If not specified, the message expiration time will be used by default.
retainer.retained_message_ttl = "0m"
```

保留消息内置模块使用内存（RAM）存储保留消息。

"max_retained_messages"：可以配置最大保留消息数量，`0` 表示无限制。
"max_payload_size"：限制消息负载大小。
"retained_message_ttl"：配置保留消息过期时间，`"0m"` 表示不过期。如果未指定，则默认情况下将使用消息过期时间。

该内置模块现在默认已**启用**。要验证或更改此设置，请检查主配置文件 `oximqtt.toml` 中的 `[retainer]` 配置段是否存在并正确配置。
