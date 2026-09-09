//! Minimal TCP transport: MQTT framing with the benchmark's own codec over a
//! split `tokio::net::TcpStream`.
//!
//! The reader and the writer are separate ownership halves so a single client
//! task can `select!` between "wait for the next packet" and "it is time to
//! publish", which is what keeps the per-connection cost low enough to reach
//! hundreds of thousands of connections.

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use bytes::{BufMut, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpSocket;
use tokio::time;

use crate::mqtt::{Codec, Packet, Version};

/// Maximum packet size accepted from the broker.
const MAX_PACKET_SIZE: usize = 4 * 1024 * 1024;

/// Bytes buffered by the writer before an automatic flush.
const FLUSH_THRESHOLD: usize = 32 * 1024;

/// Open a TCP connection, optionally bound to a local source address.
pub async fn connect(
    peer: SocketAddr,
    bind: Option<IpAddr>,
    timeout: Duration,
    version: Version,
) -> io::Result<(Reader, Writer)> {
    let socket = match peer.ip() {
        IpAddr::V4(_) => TcpSocket::new_v4()?,
        IpAddr::V6(_) => TcpSocket::new_v6()?,
    };
    if let Some(local) = bind {
        socket.bind(SocketAddr::new(local, 0))?;
    }
    let stream = time::timeout(timeout, socket.connect(peer))
        .await
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                format!("tcp connect to {peer} timed out"),
            )
        })??;
    stream.set_nodelay(true)?;

    let (rd, wr) = stream.into_split();
    Ok((
        Reader {
            stream: rd,
            buf: BytesMut::with_capacity(8 * 1024),
            codec: Codec::new(version, MAX_PACKET_SIZE),
        },
        Writer {
            stream: wr,
            buf: BytesMut::with_capacity(4 * 1024),
            codec: Codec::new(version, MAX_PACKET_SIZE),
        },
    ))
}

/// Decoder side of the transport.
pub struct Reader {
    stream: OwnedReadHalf,
    buf: BytesMut,
    codec: Codec,
}

/// Encoder side of the transport.
pub struct Writer {
    stream: OwnedWriteHalf,
    buf: BytesMut,
    codec: Codec,
}

impl Reader {
    /// Read the next complete MQTT packet, awaiting the socket as needed.
    ///
    /// Cancel safe: bytes are only taken out of the OS buffer inside one poll,
    /// never across an await point.
    pub async fn read_packet(&mut self) -> io::Result<Packet> {
        loop {
            if let Some(pkt) = self.codec.decode(&mut self.buf).map_err(decode_err)? {
                tracing::trace!(kind = pkt.packet_type(), "received packet");
                return Ok(pkt);
            }
            if self.buf.len() >= MAX_PACKET_SIZE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "incoming frame exceeds the size limit",
                ));
            }
            let want = self.codec.frame_hint().clamp(1, 16 * 1024);
            let mut tmp = vec![0u8; want];
            let n = self.stream.read(&mut tmp).await?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "connection closed",
                ));
            }
            self.buf.put_slice(&tmp[..n]);
        }
    }

    /// Drop the buffered bytes; the socket closes with the reader itself
    /// (an owned read half cannot be shut down).
    pub fn close(&mut self) {
        self.buf.clear();
    }
}

impl Writer {
    /// Encode and queue one packet; flushes automatically once the write
    /// buffer grows past [`FLUSH_THRESHOLD`].
    pub async fn send(&mut self, pkt: &Packet) -> io::Result<()> {
        self.codec.encode(pkt, &mut self.buf).map_err(encode_err)?;
        if self.buf.len() >= FLUSH_THRESHOLD {
            self.flush().await?;
        }
        Ok(())
    }

    /// Push buffered bytes to the socket.
    pub async fn flush(&mut self) -> io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let data = self.buf.split().freeze();
        self.stream.write_all(&data).await?;
        self.stream.flush().await?;
        Ok(())
    }

    /// Flush and close the write half (sends FIN).
    pub async fn close(&mut self) {
        let _ = self.flush().await;
        let _ = self.stream.shutdown().await;
    }
}

fn decode_err(e: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("decode error: {e}"))
}

fn encode_err(e: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, format!("encode error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mqtt::{Connect, Publish, QoS, SubOpts, Will};
    use bytes::Bytes;

    #[tokio::test]
    async fn packets_survive_a_real_socket_roundtrip() {
        // Loop over an actual socketpair-like pair: the framing must work when
        // bytes arrive split, coalesced and back to back.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let send = vec![
            Packet::Connect(Connect {
                username: Some("u".into()),
                password: Some(b"p".to_vec()),
                will: Some(Will {
                    topic: "w".into(),
                    payload: b"x".to_vec(),
                    qos: QoS::AtLeastOnce,
                    retain: false,
                }),
                ..Connect::new("bench-1", 30, true)
            }),
            Packet::Publish(Publish {
                topic: "a/b".into(),
                payload: Bytes::from(vec![b'p'; 5000]),
                qos: QoS::AtLeastOnce,
                retain: true,
                dup: true,
                pid: Some(4242),
            }),
            Packet::Subscribe {
                pid: 3,
                filters: vec![(
                    "a/#".into(),
                    SubOpts {
                        qos: QoS::ExactlyOnce,
                    },
                )],
            },
            Packet::PingReq,
        ];

        let tx = send.clone();
        let writer = tokio::spawn(async move {
            let peer = tokio::net::TcpStream::connect(addr).await.unwrap();
            let (_, wr) = peer.into_split();
            let mut w = Writer {
                stream: wr,
                buf: BytesMut::new(),
                codec: Codec::new(Version::V3, MAX_PACKET_SIZE),
            };
            for p in &tx {
                w.send(p).await.unwrap();
            }
            w.flush().await.unwrap();
            // Keep the socket alive until the reader is done.
            tokio::time::sleep(Duration::from_millis(300)).await;
        });

        let (peer, _) = listener.accept().await.unwrap();
        let (rd, _) = peer.into_split();
        let mut r = Reader {
            stream: rd,
            buf: BytesMut::with_capacity(64),
            codec: Codec::new(Version::V3, MAX_PACKET_SIZE),
        };
        for expected in &send {
            let got = tokio::time::timeout(Duration::from_secs(5), r.read_packet())
                .await
                .unwrap();
            assert_eq!(&got.unwrap(), expected);
        }
        writer.await.unwrap();
    }

    #[tokio::test]
    async fn reader_reports_closed_connections() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let closer = tokio::spawn(async move {
            let (peer, _) = listener.accept().await.unwrap();
            drop(peer);
        });
        let (rd, _) = tokio::net::TcpStream::connect(addr)
            .await
            .unwrap()
            .into_split();
        let mut r = Reader {
            stream: rd,
            buf: BytesMut::new(),
            codec: Codec::new(Version::V3, MAX_PACKET_SIZE),
        };
        let err = r.read_packet().await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
        closer.await.unwrap();
    }

    #[tokio::test]
    async fn connect_to_a_closed_port_fails_without_panicking() {
        let peer: SocketAddr = "127.0.0.1:1".parse().unwrap();
        assert!(connect(peer, None, Duration::from_millis(500), Version::V3)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn writer_batches_until_the_threshold() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let accept = tokio::spawn(async move {
            let (mut peer, _) = listener.accept().await.unwrap();
            let mut got = Vec::new();
            tokio::io::AsyncReadExt::read_to_end(&mut peer, &mut got)
                .await
                .unwrap();
            got
        });

        let (_, peer) = tokio::net::TcpStream::connect(addr)
            .await
            .unwrap()
            .into_split();
        let mut w = Writer {
            stream: peer,
            buf: BytesMut::new(),
            codec: Codec::new(Version::V3, MAX_PACKET_SIZE),
        };
        let big = Packet::Publish(Publish {
            topic: "t".into(),
            payload: Bytes::from(vec![b'x'; FLUSH_THRESHOLD]),
            qos: QoS::AtMostOnce,
            retain: false,
            dup: false,
            pid: None,
        });
        w.send(&big).await.unwrap(); // triggers the automatic flush
        w.send(&Packet::PingReq).await.unwrap(); // still buffered
        w.flush().await.unwrap();
        drop(w);

        let got = accept.await.unwrap();
        let big_len = crate::mqtt::publish_wire_size(Version::V3, "t", FLUSH_THRESHOLD, 0);
        assert_eq!(
            got.len(),
            big_len + 2,
            "everything must arrive after the explicit flush"
        );
    }

    #[tokio::test]
    async fn binding_to_an_unusable_local_address_fails() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let peer = listener.local_addr().unwrap();
        drop(listener);
        // 8.8.8.8 is not a local address of this host.
        let res = connect(
            peer,
            Some(IpAddr::from([8, 8, 8, 8])),
            Duration::from_millis(500),
            Version::V5,
        )
        .await;
        assert!(res.is_err());
    }
}
