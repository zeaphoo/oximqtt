//! Example: MQTT server with built-in modules.
//! Demonstrates how to initialize built-in modules (ACL, Retainer, Auth-JWT, Sys-Topic)
//! using the unified `builtins::init_all` function.

use oximqtt::{context::ServerContext, net::Builder, server::MqttServer, Result};
use simple_logger::SimpleLogger;

#[tokio::main]
async fn main() -> Result<()> {
    SimpleLogger::new().with_level(log::LevelFilter::Info).init()?;

    let scx = ServerContext::new().build().await;

    oximqtt::builtins::init_all(&scx).await?;

    MqttServer::new(scx)
        .listener(
            Builder::new()
                .name("external/tcp")
                .laddr(([0, 0, 0, 0], 1883).into())
                .allow_anonymous(false)
                .bind()?
                .tcp()?,
        )
        .build()
        .run()
        .await?;
    Ok(())
}
