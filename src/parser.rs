#![allow(unused)]
use anyhow::{Error, anyhow};
use bpfx::network::*;
use chrono::{DateTime, Utc};
use clap::builder::Str;
use futures::StreamExt;
use rdkafka::ClientConfig;
use rdkafka::producer::Producer;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::{Deserialize, Serialize};
use serde_json::Serializer;
use std::os::unix::fs::MetadataExt;
use std::sync::{Arc, RwLock, mpsc};
use std::time::Duration;
use std::{
    collections::HashMap,
    fs::{self, File, read_link},
    io::{BufRead, BufReader, Read, empty},
    os::{
        fd::{self, FromRawFd},
        unix::{fs::PermissionsExt, process},
    },
    path::{Path, PathBuf},
    time::Instant,
};
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::sleep;

pub fn get_process_name(pid: &u32) -> anyhow::Result<String> {
    let path = format!("/proc/{pid}/comm");
    let mut file = File::open(path)?;
    let mut out = String::new();
    file.read_to_string(&mut out);
    Ok(out.trim_start().trim_end().to_string())
}

pub type PidMap = Arc<RwLock<HashMap<u64, u32>>>;

pub async fn parse_proc_net_tcp(
    mut network: PollNetwork,
    sender: tokio::sync::mpsc::Sender<EventType>,
) -> anyhow::Result<()> {
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

    Ok(())
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
