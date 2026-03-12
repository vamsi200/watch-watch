#![allow(unused)]
use crate::{
    parser::{EvenType, TcpEvent, TcpState, TcpWrapper, UdpEvent, UdpWrapper},
    rules::{apply_rules_tcp, load_rules},
};
use anyhow::Context;
use rdkafka::{
    ClientConfig, Message,
    consumer::{self, Consumer, StreamConsumer},
    producer::FutureProducer,
};
use serde_json::{Map, from_value, json};
use tokio::sync::mpsc::UnboundedSender;

pub async fn consume_events(topic_name: Vec<&str>) -> anyhow::Result<(), anyhow::Error> {
    let consumer: StreamConsumer = ClientConfig::new()
        .set("group.id", "ids")
        .set("bootstrap.servers", "192.168.1.9:9092")
        .set("auto.offset.reset", "earliest")
        .create()
        .with_context(|| "Failed to create consumer")?;

    consumer
        .subscribe(&topic_name)
        .with_context(|| "Failed to subscribe to events");

    let rules = load_rules()?;
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", "192.168.1.9:9092")
        .create()
        .unwrap();

    loop {
        match consumer.recv().await {
            Ok(msg) => {
                if let Some(msg) = msg.payload() {
                    if let Ok(data) = serde_json::from_slice::<TcpWrapper>(msg) {
                        apply_rules_tcp(data.tcp_event, &rules, &producer).await?;
                    } else if let Ok(data) = serde_json::from_slice::<UdpWrapper>(msg) {
                    }
                }
            }
            Err(e) => println!("{e}"),
        }
    }

    Ok(())
}
