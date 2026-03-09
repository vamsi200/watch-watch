#![allow(unused)]

use rdkafka::ClientConfig;
use rdkafka::producer::FutureProducer;
use watch_watch::parser::{self, EvenType, TcpEvent, UdpEvent, serialize_data};
use watch_watch::producer::connect_kafka;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", "192.168.1.8:9092")
        .create()
        .unwrap();

    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<EvenType>();

    tokio::spawn(parser::parse_proc_net_tcp(sender.clone()));
    tokio::spawn(parser::parse_net_udp(sender));

    while let Some(event_type) = receiver.recv().await {
        match event_type {
            EvenType::TcpEvent(events) => {
                let key = events.local_ip.clone();
                let serialized_data = serialize_data(parser::EvenType::TcpEvent(events)).unwrap();
                connect_kafka(serialized_data, "tcp.events", &key, &producer)
                    .await
                    .unwrap();
            }
            EvenType::UdpEvent(events) => {
                let key = events.local_ip.clone();
                let serialized_data = serialize_data(parser::EvenType::UdpEvent(events)).unwrap();
                connect_kafka(serialized_data, "udp.events", &key, &producer)
                    .await
                    .unwrap();
            }
        }
    }

    Ok(())
}
