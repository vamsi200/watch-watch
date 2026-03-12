#![allow(unused)]
use crate::{
    parser::{EvenType, TcpEvent, TcpState, serialize_data},
    producer::connect_kafka,
};
use chrono::Utc;
use clap::builder::Str;
use rdkafka::{ClientConfig, producer::FutureProducer};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{BufReader, Read},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[derive(Deserialize, Debug, Clone)]
pub struct Rule {
    name: String,
    severity: Severity,
    rule_type: Type,
    threshold: usize,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct Alert {
    #[serde(rename = "@timestamp")]
    timestamp: String,
    name: String,
    severity: Severity,
    rule_type: Type,
    threshold: usize,
    event: TcpEvent, // make this generic
}

impl Rule {
    fn matches(&self, event: &TcpEvent) -> bool {
        match self.rule_type {
            Type::TxQueue => event.tx_queue > self.threshold as u32,
            Type::RxQueue => event.rx_queue > self.threshold as u32,
            Type::RemotePort => event.remote_port == self.threshold as u16,
            Type::LocalPort => event.local_port == self.threshold as u16,
            Type::PrivilegedPort => event.local_port < self.threshold as u16,
        }
    }
}

// add any custom types??
#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
enum Type {
    TxQueue,
    RemotePort,
    RxQueue,
    LocalPort,
    PrivilegedPort,
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::TxQueue => write!(f, "TxQueue"),
            Type::RemotePort => write!(f, "RemotePort"),
            Type::RxQueue => write!(f, "RxQueue"),
            Type::LocalPort => write!(f, "LocalPort"),
            Type::PrivilegedPort => write!(f, "PrivilegedPort"),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Serialize)]
enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

pub fn load_rules() -> anyhow::Result<Vec<Rule>, anyhow::Error> {
    let mut file = File::open("./simple_rules.json")?;
    let mut content = String::new();
    file.read_to_string(&mut content);
    let data = if let Ok(data) = serde_json::from_str::<Vec<Rule>>(&content) {
        data
    } else {
        panic!("invalid json")
    };

    Ok(data)
}

pub async fn apply_rules_tcp(
    tcp_event: TcpEvent,
    rules: &Vec<Rule>,
    producer: &FutureProducer,
) -> anyhow::Result<(), anyhow::Error> {
    println!("Sending alerts..");

    if let Some(rule) = rules.iter().find(|r| r.matches(&tcp_event)) {
        let timestamp = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        let key = &tcp_event.local_ip;
        // this expensive??
        let alert = Alert {
            timestamp: timestamp,
            name: rule.name.clone(),
            threshold: rule.threshold,
            event: tcp_event.clone(),
            rule_type: rule.rule_type.clone(),
            severity: rule.severity.clone(),
        };

        let serialized_data = serde_json::to_vec(&alert)?;
        connect_kafka(serialized_data, "alerts.events", key, &producer)
            .await
            .unwrap();
    }

    Ok(())
}
