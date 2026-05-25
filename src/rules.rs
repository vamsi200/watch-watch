#![allow(unused)]
use crate::{
    parser::{EvenType, TcpEvent, TcpState, UdpEvent, serialize_data},
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
    matcher: Vec<CompiledMatcher>,
    correlate: Option<CorrelationRule>,
    category: RuleCategory,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
struct CompiledMatcher {
    field: CompiledField,
    expected: FieldValue,
}

impl CompiledMatcher {
    fn matches(&self, event: &TcpEvent) -> bool {
        self.field.get(event) == self.expected
    }
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

impl CompiledField {
    fn get<'a>(&self, event: &'a TcpEvent) -> FieldValue {
        match self {
            CompiledField::RemotePort => FieldValue::U16(event.remote_port),
            CompiledField::LocalPort => FieldValue::U16(event.local_port),
            CompiledField::TxQueue => FieldValue::U32(event.tx_queue),
            CompiledField::RxQueue => FieldValue::U32(event.tx_queue),
            CompiledField::RemoteIp => FieldValue::String(event.remote_ip.to_string()),
            CompiledField::ProcessName => {
                if let Some(ev) = &event.process_name {
                    FieldValue::String(ev.to_string())
                } else {
                    FieldValue::None
                }
            }
            CompiledField::State => FieldValue::Enum(event.state.to_string()),
            CompiledField::LocalIp => FieldValue::String(event.local_ip.to_string()),
            CompiledField::Pid => {
                if let Some(ev) = event.pid {
                    FieldValue::U32(ev)
                } else {
                    FieldValue::None
                }
            }
        }
    }
}

impl TryFrom<RawRule> for Rule {
    type Error = anyhow::Error;

    fn try_from(raw: RawRule) -> anyhow::Result<Self> {
        let mut compiled = Vec::new();

        for (field, value) in raw.matcher {
            let field = match field.as_str() {
                "remote_port" => CompiledField::RemotePort,
                "local_port" => CompiledField::LocalPort,
                "tx_queue" => CompiledField::TxQueue,
                "rx_queue" => CompiledField::RxQueue,
                "pid" => CompiledField::Pid,
                "remote_ip" => CompiledField::RemoteIp,
                "local_ip" => CompiledField::LocalIp,
                _ => anyhow::bail!("unknown field: {}", field),
            };

            let expected = match value {
                serde_json::Value::Number(n) => FieldValue::U64(n.as_u64().unwrap()),
                serde_json::Value::String(s) => FieldValue::String(s),
                _ => anyhow::bail!("unsupported value type"),
            };

            compiled.push(CompiledMatcher { field, expected });
        }

        Ok(Self {
            name: raw.name,
            severity: raw.severity,
            matcher: compiled,
            correlate: raw.correlate,
            category: raw.category,
        })
    }
}

impl Rule {
    fn matches(&self, event: &TcpEvent) -> bool {
        self.matcher.iter().all(|m| m.matches(event))
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
pub fn laod_rules() -> anyhow::Result<Vec<Rule>, anyhow::Error> {
    let mut file = File::open("/home/vamsi/scripts/watch-watch/src/complex_rules.json")?;
    let mut content = String::new();
    file.read_to_string(&mut content);

    let raw_rules: Vec<RawRule> = serde_json::from_str(&content)?;

    let rules: Vec<Rule> = raw_rules
        .into_iter()
        .map(Rule::try_from)
        .collect::<Result<_, _>>()?;

    Ok(rules)
}

pub async fn apply_rules_tcp(
    rules: &Vec<Rule>,
    tcp_event: TcpEvent,
    producer: &FutureProducer,
    map: &mut CorrelationState,
) -> anyhow::Result<(), anyhow::Error> {
    let event = json!(tcp_event);

    for rule in rules {
        if !rule.matches(&tcp_event) {
            continue;
        }

        let timestamp = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let key = rule.severity.to_str();

        if let Some(group_key) = rule.group_key(&event) {
            if map.process(rule, &group_key, &event)? {
                let alert = Alert {
                    timestamp: timestamp,
                    name: &rule.name,
                    threshold: 0,
                    event: json!(tcp_event),
                    severity: rule.severity.clone(),
                };

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
fn test_load_rules() {
    let s = laod_rules();
    assert!(s.is_ok())
}
