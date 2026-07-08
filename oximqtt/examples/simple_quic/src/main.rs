//! Example: MQTT server with a QUIC (UDP) transport listener on port 9443.
//! Demonstrates how to configure TLS certificates for QUIC connections.

use oximqtt::{context::ServerContext, net::Builder, server::MqttServer, Result};
use simple_logger::SimpleLogger;

#[tokio::main]
async fn main() -> Result<()> {
    SimpleLogger::new().with_level(log::LevelFilter::Info).init()?;

    let scx = ServerContext::new().build().await;

    MqttServer::new(scx)
        .listener(
            Builder::new()
                .name("external/quic")
                .laddr(([0, 0, 0, 0], 9443).into())
                .tls_key(Some("./oximqtt-bin/oximqtt.key"))
                .tls_cert(Some("./oximqtt-bin/oximqtt.pem"))
                .bind_quic()?,
        )
        .build()
        .run()
        .await?;
    Ok(())
}
