#![allow(unused)]
use anyhow::anyhow;
use bpfx::network::*;
use std::fs::{self, File, OpenOptions, exists};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;
use ww_collector::enroll::{self, CollectorProfile};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(long, help = "server address")]
    pub server: String,
    #[arg(long, help = "token")]
    pub token: String,
}

#[derive(Serialize, Deserialize)]
struct Config {
    boot_strap_servers: Vec<String>,
    agent_id: String,
}

#[derive(Serialize, Debug, Deserialize)]
pub enum EventType {
    Connect(ConnectEvent),
    Accept(AcceptEvent),
    Close(CloseEvent),
    Bind(BindEvent),
    Listen(ListenEvent),
}

pub fn serialize_data(ev_type: EventType) -> anyhow::Result<Vec<u8>> {
    Ok(serde_json::to_vec(&ev_type)?)
}

pub async fn connect_kafka(
    data: Vec<u8>,
    topic_name: &str,
    key: &str,
    producer: &FutureProducer,
) -> anyhow::Result<()> {
    producer
        .send(
            FutureRecord::to(topic_name).payload(&data).key(key),
            Duration::from_secs(0),
        )
        .await
        .unwrap();

    Ok(())
}

pub fn project_directory() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "watch-watch", env!("CARGO_PKG_NAME"))
}

fn get_config_path(
    config: CollectorProfile,
    agent_id: String,
) -> anyhow::Result<(FutureProducer, String)> {
    if let Some(project_dirs) = project_directory() {
        let config_dir = project_dirs.config_dir();
        let config_path = config_dir.join("collector.toml");
        if !exists(config_dir)? {
            fs::create_dir_all(config_dir)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(config_path)?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;

        match toml::from_str::<Config>(&buf) {
            Ok(config) => {
                let servers = config.boot_strap_servers;
                return Ok((
                    ClientConfig::new()
                        .set("bootstrap.servers", servers.join(","))
                        .create()
                        .unwrap(),
                    config.agent_id,
                ));
            }
            Err(_) => {
                let config = Config {
                    boot_strap_servers: config.config.kafka.bootstrap_servers,
                    agent_id,
                };

                file.write_all(toml::to_string_pretty(&config)?.as_bytes())?;

                return Err(anyhow::Error::msg(
                    "Failed to parse kafka config, aborting..",
                ));
            }
        };
    }

    return Err(anyhow::Error::msg("Failed to get config directory"));
}

pub async fn send_network_events(
    mut network: PollNetwork,
    sender: tokio::sync::mpsc::Sender<EventType>,
) -> anyhow::Result<()> {
    println!("sending events..");

    loop {
        tokio::select! {
            Some(event) = network.next() => {
              match event {
                    NetworkEvent::Accept(e) => {
                        if sender.send(EventType::Accept(e)).await.is_err() {
                            return Ok(());
                        }
                    }

                    NetworkEvent::Connect(e) => {
                        if sender.send(EventType::Connect(e)).await.is_err() {
                            return Ok(());
                        }
                    }

                    NetworkEvent::Bind(e) => {
                        if sender.send(EventType::Bind(e)).await.is_err() {
                            return Ok(());
                        }
                    }

                    NetworkEvent::Close(e) => {
                        if sender.send(EventType::Close(e)).await.is_err() {
                            return Ok(());
                        }
                    }

                    NetworkEvent::Listen(e) => {
                        if sender.send(EventType::Listen(e)).await.is_err() {
                            return Ok(());
                        }
                    }

                    _ => {}
              }
            }
        }
    }

    Ok(())
}

use bpfx::{Bpfx, NetworkFilter};
use directories::ProjectDirs;
use futures::StreamExt;
use rdkafka::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = Args::parse();

    let agent = Uuid::new_v4().to_string();
    let config = enroll::enroll(&args.server, &args.token, &agent)?;

    let client = get_config_path(config, agent);

    if let Err(client) = client {
        anyhow::bail!("Failed to get kafka config, aborting..");
    }

    let (client, key) = client?;
    let (sender_tx, mut receiver_rx) = tokio::sync::mpsc::channel::<EventType>(1024);

    let mut bpfx = Bpfx::new()?;
    let mut poll = bpfx.subscribe(NetworkFilter::ALL)?;
    let _ = bpfx.run();

    tokio::spawn(send_network_events(poll, sender_tx));

    while let Some(event_type) = receiver_rx.recv().await {
        match event_type {
            EventType::Connect(events) => {
                let serialized_data = serialize_data(EventType::Connect(events)).unwrap();
                connect_kafka(serialized_data, "connect", &key, &client)
                    .await
                    .unwrap();
            }
            EventType::Accept(events) => {
                let serialized_data = serialize_data(EventType::Accept(events)).unwrap();
                connect_kafka(serialized_data, "accept", &key, &client)
                    .await
                    .unwrap();
            }
            EventType::Close(events) => {
                let serialized_data = serialize_data(EventType::Close(events)).unwrap();
                connect_kafka(serialized_data, "close", &key, &client)
                    .await
                    .unwrap();
            }
            EventType::Bind(events) => {
                let serialized_data = serialize_data(EventType::Bind(events)).unwrap();
                connect_kafka(serialized_data, "bind", &key, &client)
                    .await
                    .unwrap();
            }
            EventType::Listen(events) => {
                let serialized_data = serialize_data(EventType::Listen(events)).unwrap();
                connect_kafka(serialized_data, "listen", &key, &client)
                    .await
                    .unwrap();
            }
        }
    }

    Ok(())
}
