#![allow(unused)]

use bpfx::{Bpfx, NetworkFilter};
use clap::Parser;
use directories::ProjectDirs;
use rdkafka::ClientConfig;
use rdkafka::producer::FutureProducer;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs::{self, OpenOptions, exists};
use std::io::{Read, Write};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::sleep;
use watch_watch::consumer::consume_events;
use watch_watch::parser::{self, EventType, PidMap, serialize_data};
use watch_watch::rules::Alert;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(long, help = "start the consumer")]
    pub consumer: bool,
}

pub fn project_directory() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "watch-watch", env!("CARGO_PKG_NAME"))
}

#[derive(Serialize, Deserialize)]
struct ServerConfig {
    boot_strap_servers: Vec<String>,
    topics: Vec<String>,
}

fn get_bootstrap_servers() -> anyhow::Result<Vec<String>> {
    if let Some(project_dirs) = project_directory() {
        let config_dir = project_dirs.config_dir();
        let config_path = config_dir.join("server.toml");
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

        match toml::from_str::<ServerConfig>(&buf) {
            Ok(config) => {
                return Ok(config.boot_strap_servers);
            }
            Err(_) => {
                let server_config = ServerConfig {
                    boot_strap_servers: Vec::new(),
                    topics: Vec::new(),
                };

                file.write_all(toml::to_string_pretty(&server_config)?.as_bytes())?;

                return Err(anyhow::Error::msg(
                    "Failed to parse kafka server config, aborting..",
                ));
            }
        };
    }

    return Err(anyhow::Error::msg("Failed to get config directory"));
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let topic_list = vec!["connect", "accept", "close", "bind", "listen"];
    let boot_strap_servers = get_bootstrap_servers()?;
    consume_events(topic_list, boot_strap_servers)
        .await
        .unwrap();
    Ok(())
}
