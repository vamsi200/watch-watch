#![allow(unused)]

// connect to server and enroll thyself.
//
// expectation:
// ww-collector enroll \
// --server https://bla-bla.example.com \
// --token "$ENROLLMENT_TOKEN"

use base64::{alphabet::URL_SAFE, engine::general_purpose::URL_SAFE_NO_PAD_INDIFFERENT};
use ed25519_dalek::SigningKey;
use rand::{rand_core::UnwrapErr, rngs::SysRng};
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

pub fn enroll(
    server: &str,
    token: &str,
    agent: &str,
    public_key: String,
) -> anyhow::Result<CollectorProfile> {
    let mut stream = TcpStream::connect(server)?;

    let request = format!(
        "GET /enroll?token={token}&agent_id={agent}&public_key={public_key} HTTP/1.1\r\n\
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

pub fn generate_key_pair(agent_id: &str) -> anyhow::Result<String> {
    use base64::prelude::*;

    let mut csprng = UnwrapErr(SysRng);
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();

    let public_key = URL_SAFE_NO_PAD_INDIFFERENT.encode(verifying_key.to_bytes());
    Ok(public_key)
}
