#![allow(unused)]
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type ProfileId = String;
pub type AgentId = String;

// Could just use Configs from kafka.rs, but collector does not need those extra information
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CollectorKafkaConfig {
    pub bootstrap_servers: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CollectorConfig {
    pub version: i64,
    pub kafka: CollectorKafkaConfig,
    pub topics: TopicsConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TopicsConfig {
    pub connect: String,
    pub accept: String,
    pub close: String,
    pub bind: String,
    pub listen: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CollectorProfile {
    pub name: String,
    pub config: CollectorConfig,
}

pub struct Profile {
    pub name: String,
    pub kafka_cluster: String,
    pub version: i64,
    pub kafka_config: CollectorKafkaConfig,
    pub config: TopicsConfig,
    pub agents_enrolled: i64,
}
