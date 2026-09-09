# OXIMQTT Broker

English | [简体中文](../zh_CN/install.md)

## Install

OXIMQTT currently supports the following operating systems (prebuilt binaries are
published for Linux amd64/arm64; macOS and Windows are built from source):

### Installing the prebuilt binary (Linux amd64 / arm64)

Prebuilt Linux binaries are published on every [GitHub Release](https://github.com/zeaphoo/oximqtt/releases).
They are statically linked against musl and built by GitHub Actions, so they run
on any Linux distribution without extra runtime dependencies. macOS and Windows
are supported by building from source (see the next section) — no prebuilt
packages are published for them.

1. Download the archive for your architecture:

```bash
# x86_64 / amd64
$ wget "https://github.com/zeaphoo/oximqtt/releases/latest/download/oximqtt-linux-amd64.tar.gz"

# ARM64 / aarch64
$ wget "https://github.com/zeaphoo/oximqtt/releases/latest/download/oximqtt-linux-arm64.tar.gz"
```

   To pin a specific release, use its tag: `https://github.com/zeaphoo/oximqtt/releases/download/v0.23.0/oximqtt-linux-amd64.tar.gz`.

2. Extract it (the archive contains `oximqttd` and `oximqtt.toml` at its root):

```bash
$ mkdir -p /app/oximqtt && tar -xzf oximqtt-linux-amd64.tar.gz -C /app/oximqtt
```

3. Make the binary executable:

```bash
$ cd /app/oximqtt && chmod +x oximqttd
```

4. Start the service:

```bash
$ cd /app/oximqtt && ./oximqttd -f ./oximqtt.toml
```

5. Check the service:

```bash
$ netstat -tlnp | grep 1883
tcp        0      0 0.0.0.0:1883            0.0.0.0:*               LISTEN      3312/./oximqttd
tcp        0      0 0.0.0.0:11883           0.0.0.0:*               LISTEN      3312/./oximqttd
```

### Compile and install from source code

#### Install the RUST compilation environment

Operating in Centos7. Skip this process if the compilation environment already exists. Attention: Toolchain requires
1.89 or later versions.

1. Install Rustup

   Open first: https://rustup.rs, Then download or run the command as prompted.

   Execute in Linux:

```bash
$ curl https://sh.rustup.rs -sSf | sh
```

Make environment variables effective

```bash
$ source $HOME/.cargo/env
```

##### Compile

1. Get source code

```bash
$ git clone https://github.com/zeaphoo/oximqtt.git
```

2. Switch to the nearest tag

```bash
$ cd oximqtt
$ git checkout $(git describe --tags $(git rev-list --tags --max-count=1))
```

3. Build

```bash
$ cargo build --release
```

##### Start OXIMQTT Broker

1. Copy programs and config files

```bash
$ mkdir -p /app/oximqtt/bin
$ cp target/release/oximqttd /app/oximqtt/bin/
$ cp oximqtt.toml /app/oximqtt/etc/
$ cp oximqtt-bin/oximqtt.pem  /app/oximqtt/etc/
$ cp oximqtt-bin/oximqtt.key  /app/oximqtt/etc/
```

2. Modify the configuration(oximqtt.toml)

- Built-in modules (ACL, JWT auth, retainer, sys-topic) are configured directly in oximqtt.toml under their respective sections (e.g. `[acl]`, `[auth_jwt]`, `[retainer]`, `[sys_topic]`)
- If TLS is enabled, you can modify the listener.tls.external configuration

```bash
vi /app/oximqtt/etc/oximqtt.toml

##--------------------------------------------------------------------
## MQTT/TLS - External TLS Listener for MQTT Protocol
listener.tls.external.addr = "0.0.0.0:8883"
listener.tls.external.cert = "/app/oximqtt/etc/oximqtt.pem"
listener.tls.external.key = "/app/oximqtt/etc/oximqtt.key"

```

3. Start Service

```bash
$ cd /app/oximqtt
$ ./bin/oximqttd -f "./etc/oximqtt.toml"
```





