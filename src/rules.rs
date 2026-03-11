#![allow(unused)]
use crate::{
    parser::{EvenType, TcpEvent, TcpState, serialize_data},
    producer::connect_kafka,
};
use rdkafka::{ClientConfig, producer::FutureProducer};
use serde::Deserialize;
use std::{
    fs::File,
    io::{BufReader, Read},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[derive(Deserialize, Debug)]
pub struct Rule {
    name: String,
    severity: Severity,
    rule_type: Type,
    threshold: usize,
}

// add any custom types??
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
enum Type {
    TxQueue,
    RemotePort,
}

#[derive(Debug, Deserialize)]
enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

pub fn load_rules() -> anyhow::Result<Vec<Rule>, anyhow::Error> {
    let mut file = File::open("./tcp_rules.json")?;
    let mut content = String::new();
    file.read_to_string(&mut content);
    let data = if let Ok(data) = serde_json::from_str::<Vec<Rule>>(&content) {
        data
    } else {
        panic!("invalid json")
    };

    Ok(data)
}

pub async fn apply_rules_tcp(tcp_event: TcpEvent) -> anyhow::Result<(), anyhow::Error> {
    let rules = load_rules()?;

    let should_send = rules.iter().any(|rule| {
        matches!(rule.rule_type, Type::TxQueue) && tcp_event.tx_queue >= rule.threshold as u32
    });

    if should_send {
        println!("sending alerts..");
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", "192.168.1.9:9092")
            .create()
            .unwrap();
        let key = tcp_event.local_ip.clone();
        let serialized_data = serialize_data(EvenType::TcpEvent(tcp_event)).unwrap();
        connect_kafka(serialized_data, "alerts.events", &key, &producer)
            .await
            .unwrap();
    }

    Ok(())
}

