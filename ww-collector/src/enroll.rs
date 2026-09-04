#![allow(unused)]

// connect to server and enroll thyself.
//
// expectation:
// ww-collector enroll \
// --server https://bla-bla.example.com \
// --token "$ENROLLMENT_TOKEN"

use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    net::TcpStream,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CollectorProfile {
    pub name: String,
    pub config: CollectorConfig,
}

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

pub fn enroll(server: &str, token: &str, agent: &str) -> anyhow::Result<CollectorProfile> {
    let mut stream = TcpStream::connect(server)?;

    let request = format!(
        "GET /enroll?token={token}&agent_id={agent} HTTP/1.1\r\n\
     Host: {server}\r\n\
     Connection: close\r\n\
     \r\n"
    );

    stream.write_all(request.as_bytes())?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;

    let response = String::from_utf8_lossy(&response);
    println!("{response}");

    let (_, body) = response
        .split_once("\r\n\r\n")
        .expect("invalid HTTP response");

    let response: CollectorProfile = serde_json::from_str(&body)?;

    Ok(response)
}
