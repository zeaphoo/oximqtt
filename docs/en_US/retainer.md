English | [简体中文](../zh_CN/retainer.md)


# Retain Message


When the client sets the **retain** flag while publishing a message, the message will be retained.
Then, when the client subscribes to a topic filter that matches this message, the retained message will be received.

The **Retain Message** feature is managed by the **retainer** built-in module. Enable it by adding a `[retainer]` section to `oximqtt.toml`.

#### Built-in Module:

```bash
retainer
```

#### Configuration section in `oximqtt.toml`:

```bash
[retainer]
```

#### Configuration options:

```bash
##--------------------------------------------------------------------
## retainer (built-in module)
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

The retainer built-in module uses in-memory (RAM) storage for retained messages.

"max_retained_messages" can be configured to set the maximum number of retained messages, where `0` indicates no limit.
"max_payload_size" limits the size of message payloads.
"retained_message_ttl" configures the expiration time for retained messages. A value of `"0m"` means no expiration.
If not specified, the message expiration time will be used by default.

The module is now **enabled by default** when the `[retainer]` section is present in the main configuration `oximqtt.toml`.
