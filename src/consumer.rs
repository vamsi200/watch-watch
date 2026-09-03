#![allow(unused)]
use std::collections::VecDeque;

use crate::{
    parser::EventType,
    rules::{Alert, CorrelationState},
};
use anyhow::Context;
use rdkafka::{
    ClientConfig, Message,
    consumer::{self, Consumer, StreamConsumer},
    producer::FutureProducer,
};
use serde_json::{Map, from_value, json};

pub async fn consume_events(
    topic_name: Vec<&str>,
    boot_strap_servers: Vec<String>,
) -> anyhow::Result<(), anyhow::Error> {
    println!("Starting the consumer..");
    let consumer: StreamConsumer = ClientConfig::new()
        .set("group.id", "ids")
        .set("bootstrap.servers", boot_strap_servers.join(","))
        .set("auto.offset.reset", "earliest")
        .create()
        .with_context(|| "Failed to create consumer")?;

    consumer
        .subscribe(&topic_name)
        .with_context(|| "Failed to subscribe to events");

    // let rules = laod_rules()?;
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", "192.168.1.7:9092")
        .create()
        .unwrap();
    let mut map = CorrelationState::new();

    loop {
        match consumer.recv().await {
            Ok(msg) => {
                if let Some(msg) = msg.payload() {
                    if let Ok(data) = serde_json::from_slice::<EventType>(msg) {
                        // apply_rules_tcp(&rules, data.tcp_event, &producer, &mut map).await?;
                    }
                }
            }
            Err(e) => println!("{e}"),
        }
    }

    Ok(())
}
