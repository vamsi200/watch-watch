#![allow(unused)]
use crate::{
    parser::{EventType, serialize_data},
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
    ops::Deref,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[derive(Deserialize, Debug)]
pub struct RawRule {
    name: String,
    severity: Severity,

    #[serde(rename = "match")]
    matcher: HashMap<String, serde_json::Value>,

    correlate: Option<CorrelationRule>,
    category: RuleCategory,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct Rule {
    name: String,
    severity: Severity,
    #[serde(rename = "match")]
    correlate: Option<CorrelationRule>,
    category: RuleCategory,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct Alert<'a> {
    #[serde(rename = "@timestamp")]
    timestamp: String,
    name: &'a str,
    severity: Severity,
    threshold: usize,
    event: Value,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum CompiledField {
    RemotePort,
    LocalPort,
    TxQueue,
    RxQueue,
    State,
    ProcessName,
    LocalIp,
    RemoteIp,
    Pid,
}

#[derive(Deserialize, Debug, Clone, Serialize, PartialEq)]
enum FieldValue {
    String(String),
    U16(u16),
    U32(u32),
    U64(u64),
    Bool(bool),
    Enum(String),
    None,
}

impl Rule {}

#[derive(Debug, Clone)]
pub struct WindowEntry {
    pub timestamp_ms: i64,
    pub event: Value,
}

pub struct CorrelationState {
    pub windows: HashMap<(String, String), VecDeque<WindowEntry>>,
}

impl CorrelationState {
    pub fn new() -> Self {
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
        let entry = self
            .windows
            .entry((rule.name.clone(), group_key.to_string()))
            .or_default();

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

        Ok(Self::evaluate(correlate, &entry))
    }

    fn evaluate(correlate: &CorrelationRule, entry: &VecDeque<WindowEntry>) -> bool {
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

#[derive(Deserialize, Debug, Serialize, Clone)]
enum RuleCategory {
    Recon,
    Persistence,
    LateralMovement,
    Exfiltration,
    PrivEsc,
}

enum RuleStatus {
    Enable,
    Disable,
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

// pub fn laod_rules() -> anyhow::Result<Vec<Rule>, anyhow::Error> {
//     let mut file = File::open("/home/vamsi/scripts/watch-watch/src/complex_rules.json")?;
//     let mut content = String::new();
//     file.read_to_string(&mut content);
//
//     let raw_rules: Vec<RawRule> = serde_json::from_str(&content)?;
//
//     let rules: Vec<Rule> = raw_rules
//         .into_iter()
//         .map(Rule::try_from)
//         .collect::<Result<_, _>>()?;
//
//     Ok(rules)
// }
//
// #[test]
// fn test_load_rules() {
//     let s = laod_rules();
//     assert!(s.is_ok())
// }
