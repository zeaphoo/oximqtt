# OXIMQTT Broker

[English](../en_US/install.md)  | 简体中文

## 安装

OXIMQTT 目前支持的操作系统(Linux 发布预编译 amd64/arm64 包;macOS / Windows
需源码编译):

### 预编译二进制安装(Linux amd64 / arm64)

每个 [GitHub Release](https://github.com/zeaphoo/oximqtt/releases) 都会发布 Linux
预编译包。产物由 GitHub Actions 使用 musl 静态编译，不依赖系统运行库，可在任意
Linux 发行版上直接运行。macOS / Windows 请使用下一节的源码编译方式，官方不发布
这两个平台的预编译包。

1. 按 CPU 架构下载压缩包:

```bash
# x86_64 / amd64
$ wget "https://github.com/zeaphoo/oximqtt/releases/latest/download/oximqtt-linux-amd64.tar.gz"

# ARM64 / aarch64
$ wget "https://github.com/zeaphoo/oximqtt/releases/latest/download/oximqtt-linux-arm64.tar.gz"
```

   如需固定某个版本，将 URL 换成对应 tag: `https://github.com/zeaphoo/oximqtt/releases/download/v0.23.0/oximqtt-linux-amd64.tar.gz`。

2. 解压(压缩包根目录即 `oximqttd` 与 `oximqtt.toml`):

```bash
$ mkdir -p /app/oximqtt && tar -xzf oximqtt-linux-amd64.tar.gz -C /app/oximqtt
```

3. 赋予执行权限:

```bash
$ cd /app/oximqtt && chmod +x oximqttd
```

4. 启动服务:

```bash
$ cd /app/oximqtt && ./oximqttd -f ./oximqtt.toml
```

5. 查看服务:

```bash
$ netstat -tlnp | grep 1883
tcp        0      0 0.0.0.0:1883            0.0.0.0:*               LISTEN      3312/./oximqttd
tcp        0      0 0.0.0.0:11883           0.0.0.0:*               LISTEN      3312/./oximqttd
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
$ ./bin/oximqttd -f "./etc/oximqtt.toml"
```















