# OXIMQTT Broker

English | [简体中文](../zh_CN/install.md)

## Install

OXIMQTT Currently supported operating systems:

- Linux
- macOS
- Windows Server

### Installing via ZIP Binary Package (Linux、MacOS、Windows)

Get the binary package of the corresponding OS from [OXIMQTT Download](https://github.com/zeaphoo/oximqtt/releases) page.

1. Download the ZIP package from [GitHub Release](https://github.com/zeaphoo/oximqtt/releases).

```bash
$ wget "https://github.com/zeaphoo/oximqtt/releases/download/0.21.0/oximqtt-0.21.0-x86_64-unknown-linux-musl.zip"
```

2. Decompress the zip package you downloaded from [GitHub Release](https://github.com/zeaphoo/oximqtt/releases).

```bash
$ unzip oximqtt-0.21.0-x86_64-unknown-linux-musl.zip -d /app/
```

3. Modify the permissions

```bash
$ cd /app/oximqtt
$ chmod +x bin/oximqttd
```

4. Start the service

```bash
$ cd /app/oximqtt
$ sh start.sh
```

5. Check the service

```bash
$ netstat -tlnp|grep 1883
tcp        0      0 0.0.0.0:1883            0.0.0.0:*               LISTEN      3312/./bin/oximqttd
tcp        0      0 0.0.0.0:11883           0.0.0.0:*               LISTEN      3312/./bin/oximqttd
```

### Compile and install from source code

#### Install the RUST compilation environment

Operating in Centos7. Skip this process if the compilation environment already exists. attention: Toolchain requires
1.56 or later versions. If connection errors are reported in 1.59 or later versions, upgrade the system development
environment.

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
./bin/oximqttd -f "./etc/oximqtt.toml"
```





