#![allow(unused)]

use bpfx::{Bpfx, NetworkFilter};
use clap::Parser;
use rdkafka::ClientConfig;
use rdkafka::producer::FutureProducer;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::sleep;
use watch_watch::consumer::consume_events;
use watch_watch::parser::{self, EventType, PidMap, serialize_data};
use watch_watch::producer::connect_kafka;
use watch_watch::rules::Alert;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(long, help = "start the producer")]
    pub producer: bool,

    #[arg(long, help = "start the consumer")]
    pub consumer: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = Args::parse();
    if args.consumer {
        let topic_list = vec!["connect", "accept", "close", "bind", "listen"];
        consume_events(topic_list).await.unwrap();
    }

    if args.producer {
        println!("Starting the producer..");
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", "192.168.1.7:9092")
            .create()
            .unwrap();

        let (sender_tx, mut receiver_rx) = tokio::sync::mpsc::channel::<EventType>(1024);

        let mut bpfx = Bpfx::new()?;
        let mut poll = bpfx.subscribe(NetworkFilter::ALL)?;
        let _ = bpfx.run();

        tokio::spawn(parser::parse_proc_net_tcp(poll, sender_tx));

        while let Some(event_type) = receiver_rx.recv().await {
            match event_type {
                EventType::Connect(events) => {
                    let key = events.endpoints.local_ip.to_string();
                    let serialized_data =
                        serialize_data(parser::EventType::Connect(events)).unwrap();
                    connect_kafka(serialized_data, "connect", &key, &producer)
                        .await
                        .unwrap();
                }
                EventType::Accept(events) => {
                    let key = events.endpoints.local_ip.to_string();
                    let serialized_data = serialize_data(EventType::Accept(events)).unwrap();
                    connect_kafka(serialized_data, "accept", &key, &producer)
                        .await
                        .unwrap();
                }
                EventType::Close(events) => {
                    let key = events.endpoints.local_ip.to_string();
                    let serialized_data = serialize_data(parser::EventType::Close(events)).unwrap();
                    connect_kafka(serialized_data, "close", &key, &producer)
                        .await
                        .unwrap();
                }
                EventType::Bind(events) => {
                    let key = events.endpoints.local_ip.to_string();
                    let serialized_data = serialize_data(parser::EventType::Bind(events)).unwrap();
                    connect_kafka(serialized_data, "bind", &key, &producer)
                        .await
                        .unwrap();
                }
                EventType::Listen(events) => {
                    let key = events.endpoints.local_ip.to_string();
                    let serialized_data =
                        serialize_data(parser::EventType::Listen(events)).unwrap();
                    connect_kafka(serialized_data, "listen", &key, &producer)
                        .await
                        .unwrap();
                }
            }
        }
    }

    Ok(())
}
