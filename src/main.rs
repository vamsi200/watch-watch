#![allow(unused)]

use clap::Parser;
use rdkafka::ClientConfig;
use rdkafka::producer::FutureProducer;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::sleep;
use watch_watch::consumer::consume_events;
use watch_watch::parser::{
    self, EvenType, PidMap, TcpEvent, UdpEvent, build_pid_map, serialize_data,
};
use watch_watch::producer::connect_kafka;
use watch_watch::rules::{apply_rules_tcp, load_rules};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(long, help = "start the producer")]
    pub producer: bool,

    #[arg(long, help = "start the consumer")]
    pub consumer: bool,
}

async fn refresh_pid_map(map: PidMap) -> anyhow::Result<()> {
    println!("Refreshing Pid map..");
    loop {
        let new_map = build_pid_map()?;
        {
            let mut guard = map.write().unwrap();
            *guard = new_map;
        }
        sleep(Duration::from_secs(30));
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = Args::parse();
    let (r_sender, mut r_receiver) = unbounded_channel::<TcpEvent>();

    if args.consumer {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<EvenType>();
        let topic_list = vec!["tcp.events", "udp.events"];
        consume_events(topic_list).await.unwrap();
    }

    if args.producer {
        println!("Starting the producer..");
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", "192.168.1.9:9092")
            .create()
            .unwrap();

        let pid_map: PidMap = Arc::new(RwLock::new(HashMap::new()));

        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<EvenType>();

        tokio::spawn(refresh_pid_map(pid_map.clone()));
        tokio::spawn(parser::parse_proc_net_tcp(sender.clone(), pid_map.clone()));
        tokio::spawn(parser::parse_net_udp(sender, pid_map));

        while let Some(event_type) = receiver.recv().await {
            match event_type {
                EvenType::TcpEvent(events) => {
                    let key = events.local_ip.clone();
                    let serialized_data =
                        serialize_data(parser::EvenType::TcpEvent(events)).unwrap();
                    connect_kafka(serialized_data, "tcp.events", &key, &producer)
                        .await
                        .unwrap();
                }
                EvenType::UdpEvent(events) => {
                    let key = events.local_ip.clone();
                    let serialized_data =
                        serialize_data(parser::EvenType::UdpEvent(events)).unwrap();
                    connect_kafka(serialized_data, "udp.events", &key, &producer)
                        .await
                        .unwrap();
                }
            }
        }
    }

    Ok(())
}
