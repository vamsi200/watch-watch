#![allow(unused)]

use bpfx::{Bpfx, NetworkFilter};
use chrono::{DateTime, Days, Utc};
use clap::Parser;
use directories::ProjectDirs;
use nanoid::alphabet::SAFE;
use rdkafka::ClientConfig;
use rdkafka::producer::FutureProducer;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::fs::{self, OpenOptions, create_dir_all, exists};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::sleep;
use watch_watch::config::{
    CollectorConfig, CollectorKafkaConfig, CollectorProfile, Profile, TopicsConfig,
};
use watch_watch::consumer::consume_events;
use watch_watch::db::{create_tables, fetch_profiles, fetch_tokens, table_exists};
use watch_watch::parser::{self, EventType, PidMap, serialize_data};
use watch_watch::reg::{
    create_collector_profile, create_enrollment_token, create_profile, get_collector_profile,
    validate_token,
};
use watch_watch::rules::Alert;
use watch_watch::server::{Server, start_server};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(long, help = "start the consumer")]
    pub consumer: bool,
}

pub fn project_directory() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "watch-watch", env!("CARGO_PKG_NAME"))
}

pub fn get_db(project_dirs: &ProjectDirs) -> anyhow::Result<PathBuf> {
    let path = project_dirs.data_dir().to_path_buf();
    if !exists(&path)? {
        create_dir_all(&path);
        OpenOptions::new().create(true).open(path.join("test.db"))?;
    }
    Ok(path)
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

fn init(connection: &Connection) -> anyhow::Result<()> {
    create_tables(&connection)?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // let topic_list = vec!["connect", "accept", "close", "bind", "listen"];
    // let boot_strap_servers = get_bootstrap_servers()?;
    // consume_events(topic_list, boot_strap_servers)
    //     .await
    //     .unwrap();

    let server = Server {
        addr: "0.0.0.0",
        port: 8092,
    };

    start_server(server).await;

    // let path = get_db(&project_directory().unwrap())?;
    // //
    // let connection = Connection::open(path.join("test.db"))?;
    //
    // let collector_kafka_config = CollectorKafkaConfig {
    //     bootstrap_servers: vec![String::from("192.168.1.7:9092")],
    // };
    //
    // let topics_config = TopicsConfig::default();
    //
    // let profile = Profile {
    //     name: String::from("production"),
    //     kafka_cluster: String::from("cluster-01"),
    //     version: 1,
    //     kafka_config: collector_kafka_config,
    //     config: topics_config,
    //     agents_enrolled: 0,
    // };
    //
    // init(&connection)?;
    // create_profile(&connection, profile)?;
    //
    // let s = fetch_profiles(&connection)?;
    // println!("{s:#?}");
    //
    // let expiration = Utc::now().checked_add_days(Days::new(1)).unwrap();
    //
    // let (token, id) =
    //     create_enrollment_token(&connection, s.get(0).unwrap().name.clone(), expiration, 10)?;
    //
    // println!("Token: {token}");
    //
    // let profile_id = s.get(0).unwrap().name.clone();
    //
    // let collector_profile = CollectorProfile {
    //     name: profile_id.clone(),
    //     config: CollectorConfig {
    //         version: s.get(0).unwrap().version,
    //         kafka: s.get(0).unwrap().kafka_config.clone(),
    //         topics: s.get(0).unwrap().config.clone(),
    //     },
    // };

    // println!("{:?}", fetch_tokens(&connection));
    // create_collector_profile(&connection, collector_profile, profile_id.clone(), id)?;
    //
    // println!(
    //     "{:?}",
    //     validate_token(&String::from("DZOhzNwswp4-TH6eWuuCQQ5JEadxUn-y"))
    // );
    //
    // println!("{:#?}", get_collector_profile(&profile_id));
    //
    // connection.close();
    Ok(())
}
