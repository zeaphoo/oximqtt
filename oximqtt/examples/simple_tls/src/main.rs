//! Example: MQTT server with TLS encrypted TCP transport on port 8883.
//! Demonstrates how to configure TLS key and certificate files.

use oximqtt::{context::ServerContext, net::Builder, server::MqttServer, Result};
use simple_logger::SimpleLogger;

#[tokio::main]
async fn main() -> Result<()> {
    SimpleLogger::new().with_level(log::LevelFilter::Info).init()?;

    let scx = ServerContext::new().build().await;

    MqttServer::new(scx)
        .listener(
            Builder::new()
                .name("external/tls")
                .laddr(([0, 0, 0, 0], 8883).into())
                .tls_key(Some("./oximqtt-bin/oximqtt.key"))
                .tls_cert(Some("./oximqtt-bin/oximqtt.pem"))
                .bind()?
                .tls()?,
        )
        .build()
        .run()
        .await?;
    Ok(())
}
