#![allow(unused)]
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type ProfileId = usize;
pub type AgentId = String;

// Could just use Configs from kafka.rs, but collector does not need those extra information
#[derive(Debug, Serialize, Deserialize)]
pub struct CollectorKafkaConfig {
    pub bootstrap_servers: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectorConfig {
    pub version: u64,
    pub kafka: CollectorKafkaConfig,
    pub topics: TopicsConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TopicsConfig {
    pub connect: String,
    pub accept: String,
    pub close: String,
    pub bind: String,
    pub listen: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectorProfile {
    pub name: String,
    pub config: CollectorConfig,
}

pub fn create_profile() -> anyhow::Result<CollectorProfile> {
    todo!()
}
