#![allow(unused)]
use crate::{
    parser::{EvenType, TcpEvent, TcpState, serialize_data},
    producer::connect_kafka,
};
use anyhow::Error;
use chrono::{DateTime, Utc};
use clap::builder::Str;
use rdkafka::{ClientConfig, producer::FutureProducer};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::File,
    io::{BufReader, Read},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct Rule {
    name: String,
    severity: Severity,
    #[serde(rename = "match")]
    matcher: HashMap<String, serde_json::Value>,
    correlate: Option<CorrelationRule>,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct Alert {
    #[serde(rename = "@timestamp")]
    timestamp: String,
    name: String,
    severity: Severity,
    threshold: usize,
    event: Value,
}

impl Rule {
    fn matches(&self, event: &Value) -> bool {
        self.matcher
            .iter()
            .all(|(field, val)| event.get(field).map_or(false, |s| s == val))
    }

    fn group_key(&self, event: &Value) -> Option<String> {
        let field = &self.correlate.as_ref()?.group_by;
        event.get(field).map(|v| v.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct WindowEntry {
    pub timestamp_ms: i64,
    pub event: Value,
}

pub struct CorrelationState {
    pub windows: HashMap<(String, String), VecDeque<WindowEntry>>,
}

impl CorrelationState {
    fn new() -> Self {
        Self {
            windows: HashMap::new(),
        }
    }

    pub fn process(&mut self, rule: &Rule, group_key: &str, event: &Value) -> anyhow::Result<bool> {
        let correlate = match &rule.correlate {
            Some(c) => c,
            None => return Ok(false),
        };

        let now = event
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.timestamp_millis())
            .ok_or_else(|| anyhow::anyhow!("missing or invalid timestamp"))?;

        let window_ms = correlate.window_secs * 1_000;
        let mut entry = self
            .windows
            .entry((rule.name.clone(), group_key.to_string()))
            .or_default()
            .clone();

        while let Some(front) = entry.front() {
            if (now - front.timestamp_ms) as u64 > window_ms {
                entry.pop_front();
            } else {
                break;
            }
        }

        entry.push_back(WindowEntry {
            timestamp_ms: now,
            event: event.clone(),
        });

        Ok(self.evaluate(correlate, &entry))
    }

    fn evaluate(&self, correlate: &CorrelationRule, entry: &VecDeque<WindowEntry>) -> bool {
        match correlate.operator {
            CorrelationOperator::Count => entry.len() >= correlate.threshold,

            CorrelationOperator::Unique => {
                let unique: HashSet<String> = entry
                    .iter()
                    .filter_map(|e| e.event.get(&correlate.group_by))
                    .map(|v| v.to_string())
                    .collect();
                unique.len() >= correlate.threshold
            }

            CorrelationOperator::Sum => false,

            CorrelationOperator::Sequence => false,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CorrelationRule {
    operator: CorrelationOperator,
    group_by: String,
    threshold: usize,
    window_secs: u64,
    sum_field: Option<String>,
    steps: Option<Vec<String>>,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum CorrelationOperator {
    Count,
    Unique,
    Sequence,
    Sum,
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
    Correlation,
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::TxQueue => write!(f, "TxQueue"),
            Type::RemotePort => write!(f, "RemotePort"),
            Type::RxQueue => write!(f, "RxQueue"),
            Type::LocalPort => write!(f, "LocalPort"),
            Type::PrivilegedPort => write!(f, "PrivilegedPort"),
            Type::Correlation => write!(f, "Correlation"),
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

impl Severity {
    fn to_str(&self) -> &str {
        match self {
            Severity::Low => "Low",
            Severity::Medium => "Medium",
            Severity::High => "High",
            Severity::Critical => "Critical",
        }
    }
}
pub fn laod_rules() -> anyhow::Result<Vec<Rule>, anyhow::Error> {
    let mut file = File::open("/home/vamsi/scripts/watch-watch/src/complex_rules.json")?;
    let mut content = String::new();
    file.read_to_string(&mut content);

    let data = if let Ok(data) = serde_json::from_str::<Vec<Rule>>(&content) {
        data
    } else {
        panic!("invalid json - complex_rules")
    };

    Ok(data)
}

pub async fn apply_simple_rules_tcp(
    rules: &Vec<Rule>,
    tcp_event: TcpEvent,
    producer: &FutureProducer,
    state: &mut VecDeque<Alert>,
) -> anyhow::Result<(), anyhow::Error> {
    let mut map = CorrelationState::new();
    for rule in rules {
        if !rule.matches(&json!(tcp_event)) {
            continue;
        }
        let timestamp = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        let key = rule.severity.to_str();

        // this expensive??
        let alert = Alert {
            timestamp: timestamp,
            name: rule.name.clone(),
            threshold: 0,
            event: json!(tcp_event),
            severity: rule.severity.clone(),
        };

        if let Some(cr) = &rule.correlate {
            if map.process(rule, &cr.group_by, &json!(tcp_event))? {
                println!("Sending alerts..");
                let serialized_data = serde_json::to_vec(&alert)?;
                connect_kafka(serialized_data, "alerts.events", key, &producer)
                    .await
                    .unwrap();
            }
        }
    }

    Ok(())
}

#[test]
fn stupid_test() {
    let mut matcher: HashMap<String, serde_json::Value> = HashMap::new();
    matcher.insert("remote_port".to_string(), json!(12));

    let correlate = CorrelationRule {
        group_by: "remote_port".to_string(),
        operator: CorrelationOperator::Count,
        threshold: 5,
        window_secs: 10,
        sum_field: None,
        steps: None,
    };
    let rule = Rule {
        name: String::from("test"),
        severity: Severity::Medium,
        correlate: Some(correlate),
        matcher,
    };

    let value = json!(TcpEvent {
        local_ip: String::from("127.0.0.1"),
        local_port: 22,
        remote_ip: "10".to_string(),
        remote_port: 12,
        state: TcpState::Established,
        pid: None,
        process_name: None,
        tx_queue: 10,
        rx_queue: 32,
    });

    assert_ne!(rule.group_key(&json!(value)), None);
    assert_eq!(rule.matches(&json!(value)), true);
}

#[test]
fn test_load_rules() {
    let s = laod_rules();
    assert!(s.is_ok())
}
