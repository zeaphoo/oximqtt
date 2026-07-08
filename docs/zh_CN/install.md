# OXIMQTT Broker

[English](../en_US/install.md)  | 简体中文

## 安装

OXIMQTT 目前支持的操作系统:

- Linux
- macOS
- Windows Server

### ZIP 压缩包安装(Linux、MacOS、Windows)

需从 [GitHub Release](https://github.com/zeaphoo/oximqtt/releases) 页面获取相应操作系统的二进制软件包。

1. 从[GitHub Release](https://github.com/zeaphoo/oximqtt/releases) 下载zip包。

```bash
$ wget "https://github.com/zeaphoo/oximqtt/releases/download/0.22.0/oximqtt-0.22.0-x86_64-unknown-linux-musl.zip"
```

2. 解压从[GitHub Release](https://github.com/zeaphoo/oximqtt/releases) 下载的zip包。

```bash
$ unzip oximqtt-0.22.0-x86_64-unknown-linux-musl.zip -d /app/
```

3. 修改权限

```bash
$ cd /app/oximqtt
$ chmod +x bin/oximqttd
```

4. 启动服务

```bash
$ cd /app/oximqtt
$ sh start.sh
```

5. 查看服务

```bash
$ netstat -tlnp|grep 1883
tcp        0      0 0.0.0.0:1883            0.0.0.0:*               LISTEN      3312/./bin/oximqttd
tcp        0      0 0.0.0.0:11883           0.0.0.0:*               LISTEN      3312/./bin/oximqttd
```

### 源码编译安装

#### 安装rust编译环境

以Centos7为例，如果编译环境已经存在跳过此过程。注意：工具链需要1.89及之后版本。

1. 安装 Rustup

   先打开 Rustup 的官网：https://rustup.rs ,然后根据提示下载或运行命令。

   Linux 下执行：

```bash
$ curl https://sh.rustup.rs -sSf | sh
```

执行source $HOME/.cargo/env 让环境变量生效

```bash
$ source $HOME/.cargo/env
```

2. 配置crate.io镜像

可以在$HOME/.cargo/下建立一个config文件，加入如下配置：

```bash
$ vi $HOME/.cargo/config

[source.crates-io]
registry = "https://github.com/rust-lang/crates.io-index"
replace-with = 'tuna'

[source.tuna]
registry = "https://mirrors.tuna.tsinghua.edu.cn/git/crates.io-index.git"

[source.ustc]
registry = "git://mirrors.ustc.edu.cn/crates.io-index"

[source.sjtu]
registry = "https://mirrors.sjtug.sjtu.edu.cn/git/crates.io-index"

[source.rustcc]
registry = "git://crates.rustcc.cn/crates.io-index"

[net]
git-fetch-with-cli = true
```

如果tuna也太慢可以使用sjtu或ustc替换重试

##### 编译

1. 获取源码

```bash
$ git clone https://github.com/zeaphoo/oximqtt.git
```

2. 切换到最近的 Tag

```bash
$ cd oximqtt
$ git checkout $(git describe --tags $(git rev-list --tags --max-count=1))
```

3. 构建

```bash
$ cargo build --release
```

##### 启动OXIMQTT Broker

1. 复制程序和配置文件

```bash
$ mkdir -p /app/oximqtt/bin && mkdir -p /app/oximqtt/etc
$ cp target/release/oximqttd /app/oximqtt/bin/
$ cp oximqtt.toml /app/oximqtt/etc/
$ cp oximqtt-bin/oximqtt.pem  /app/oximqtt/etc/
$ cp oximqtt-bin/oximqtt.key  /app/oximqtt/etc/
```

2. 修改配置(oximqtt.toml)

- 根据需要启用内置模块，直接在 oximqtt.toml 中配置相应模块的参数（如 `[acl]`、`[auth_jwt]`、`[retainer]`、`[sys_topic]`）
- 如果需要启动TLS，可修改listener.tls.external配置

```bash
vi /app/oximqtt/etc/oximqtt.toml

##--------------------------------------------------------------------
## MQTT/TLS - External TLS Listener for MQTT Protocol
listener.tls.external.addr = "0.0.0.0:8883"
listener.tls.external.cert = "/app/oximqtt/etc/oximqtt.pem"
listener.tls.external.key = "/app/oximqtt/etc/oximqtt.key"
```

3. 启动服务

```bash
$ cd /app/oximqtt
./bin/oximqttd -f "./etc/oximqtt.toml"
```















