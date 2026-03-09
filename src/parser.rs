#![allow(unused)]
use anyhow::{Error, anyhow};
use avro_rs::{Reader, Writer, schema, types::Record};
use chrono::{DateTime, Utc};
use rdkafka::ClientConfig;
use rdkafka::producer::Producer;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::Serialize;
use std::os::unix::fs::MetadataExt;
use std::sync::{Arc, RwLock};
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

#[derive(Debug, Serialize)]
pub struct TcpEvent {
    pub timestamp: i64,
    pub local_ip: String,
    pub local_port: u16,
    pub remote_ip: String,
    pub remote_port: u16,
    pub state: TcpState,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
    pub tx_queue: u32,
    pub rx_queue: u32,
}

#[derive(Debug, Serialize)]
pub struct UdpEvent {
    pub timestamp: i64,
    pub local_ip: String,
    pub local_port: u16,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
}

#[derive(Debug, PartialEq, Serialize)]
pub enum TcpState {
    Established,
    Listen,
    SynSent,
    SynRecv,
    FinWait1,
    FinWait2,
    Close,
    CloseWait,
    LastAck,
    TimeWait,
    Closing,
    Unknown,
}

pub fn tcp_state_name(state: u64) -> TcpState {
    match state {
        0x01 => TcpState::Established,
        0x02 => TcpState::SynSent,
        0x03 => TcpState::SynRecv,
        0x04 => TcpState::FinWait1,
        0x05 => TcpState::FinWait2,
        0x06 => TcpState::TimeWait,
        0x07 => TcpState::Close,
        0x08 => TcpState::CloseWait,
        0x09 => TcpState::LastAck,
        0x0A => TcpState::Listen,
        0x0B => TcpState::Closing,
        _ => TcpState::Unknown,
    }
}

pub fn parse_ip(ip: &str) -> anyhow::Result<(String, u16)> {
    let mut sp = ip.split(":");
    let s = sp.next().ok_or(anyhow!("invalid ip"))?;

    let ip_bytes = (0..4)
        .map(|x| u8::from_str_radix(&s[2 * x..2 * x + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    let ip = [ip_bytes[3], ip_bytes[2], ip_bytes[1], ip_bytes[0]];
    let port_hex_value = sp.next().ok_or(anyhow!("invalid port"))?;
    let port = u16::from_str_radix(port_hex_value, 16)?;
    let ip = format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]);
    Ok((ip, port))
}

pub fn parse_queue(queue: &str) -> anyhow::Result<(u32, u32), Error> {
    let mut split = queue.split(":");
    let p_s = split.next().ok_or(anyhow!("invalid queue"))?;
    let tx_queue = u32::from_str_radix(p_s, 16)?;
    let p_s = split.next().ok_or(anyhow!("invalid queue"))?;
    let rx_queue = u32::from_str_radix(p_s, 16)?;

    Ok((tx_queue, rx_queue))
}

pub fn build_pid_map() -> anyhow::Result<HashMap<u64, u32>> {
    let mut map = HashMap::new();

    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        let pid_str = entry.file_name().to_str().unwrap().to_string();

        if !pid_str.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let pid: u32 = match pid_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let fd_dir = entry.path().join("fd");

        if !fd_dir.is_dir() {
            continue;
        }

        for fd in fs::read_dir(fd_dir)? {
            let fd = fd?;

            if let Ok(target) = fs::read_link(fd.path()) {
                let s = target.to_string_lossy();

                if let Some(inode_str) =
                    s.strip_prefix("socket:[").and_then(|v| v.strip_suffix("]"))
                {
                    if let Ok(inode) = inode_str.parse::<u64>() {
                        map.insert(inode, pid);
                    }
                }
            }
        }
    }

    Ok(map)
}

pub fn get_process_name(pid: &u32) -> anyhow::Result<String> {
    let path = format!("/proc/{pid}/comm");
    let mut file = File::open(path)?;
    let mut out = String::new();
    file.read_to_string(&mut out);
    Ok(out.trim_start().trim_end().to_string())
}

pub type PidMap = Arc<RwLock<HashMap<u64, u32>>>;

pub async fn parse_proc_net_tcp(
    sender: UnboundedSender<EvenType>,
    pid_map: PidMap,
) -> anyhow::Result<()> {
    println!("Sending Tcp Events..");
    loop {
        let mut file = File::open("/proc/net/tcp")?;
        let mut reader = BufReader::new(file);
        for line in reader.lines().skip(1) {
            let line = line?;
            let mut fs: Vec<&str> = line.trim().split_whitespace().collect();
            let (local_ip, local_port) = parse_ip(fs[1])?;
            let (remote_ip, remote_port) = parse_ip(fs[2])?;
            let state = u64::from_str_radix(fs[3], 16)?;
            let tcp_state = tcp_state_name(state);
            let (tx_queue, rx_queue) = parse_queue(fs[4])?;
            let time_stamp = if tcp_state == TcpState::Established {
                Utc::now().timestamp_millis()
            } else {
                continue;
            };

            let (mut pid, mut process_name) = (None, None);

            let inode = fs[9].parse::<u64>()?;
            let pid_map = pid_map.read().unwrap();
            if let Some(s) = pid_map.iter().find(|s| *s.0 == inode) {
                pid = Some(s.1.clone());
                process_name = Some(get_process_name(s.1)?);
            }

            let tcp_ev = TcpEvent {
                local_ip,
                local_port,
                remote_ip,
                remote_port,
                state: tcp_state,
                tx_queue,
                rx_queue,
                timestamp: time_stamp,
                pid,
                process_name,
            };
            sender.send(EvenType::TcpEvent(tcp_ev));
        }
        println!("Tcp channel Sleeping `5`s..");
        sleep(Duration::from_secs(5)).await;
    }

    Ok(())
}

pub async fn parse_net_udp(
    sender: UnboundedSender<EvenType>,
    pid_map: PidMap,
) -> anyhow::Result<()> {
    println!("Sending Udp Events..");
    loop {
        let mut file = File::open("/proc/net/udp")?;
        let mut reader = BufReader::new(&mut file);

        for line in reader.lines().skip(1) {
            let line = line?;
            let fs: Vec<&str> = line.trim().split_whitespace().collect();
            let (local_ip, local_port) = parse_ip(fs[1])?;
            let state = u64::from_str_radix(fs[3], 16)?;
            let tcp_state = tcp_state_name(state);
            let (tx_queue, rx_queue) = parse_queue(fs[4])?;
            let time_stamp = Utc::now().timestamp_millis();

            let (mut pid, mut process_name) = (None, None);

            let inode = fs[9].parse::<u64>()?;
            let pid_map = pid_map.read().unwrap();
            if let Some(s) = pid_map.iter().find(|s| *s.0 == inode) {
                pid = Some(s.1.clone());
                process_name = Some(get_process_name(s.1)?);
            }

            let ev = UdpEvent {
                local_ip,
                local_port,
                pid,
                process_name,
                timestamp: time_stamp,
            };

            sender.send(EvenType::UdpEvent(ev));
        }
        println!("Udp channel Sleeping `5`s..");
        sleep(Duration::from_secs(5)).await;
    }
    Ok(())
}

#[derive(Serialize, Debug)]
pub enum EvenType {
    TcpEvent(TcpEvent),
    UdpEvent(UdpEvent),
}

pub fn serialize_data(ev_type: EvenType) -> anyhow::Result<Vec<u8>> {
    let raw_schema = match ev_type {
        EvenType::TcpEvent(ref ev) => {
            r#"
         {
    "type": "record",
    "name": "TcpEvent",
    "fields": [
        {
        "name": "timestamp",
        "type": "long"
        },
        {
        "name": "local_ip",
        "type": "string"
        },
        {
        "name": "local_port",
        "type": "int"
        },
        {
        "name": "remote_ip",
        "type": "string"
        },
        {
        "name": "remote_port",
        "type": "int"
        },
        {
        "name": "state",
        "type": {
            "type": "enum",
            "name": "TcpState",
            "symbols": [
            "Established",
            "SynSent",
            "SynRecv",
            "FinWait1",
            "FinWait2",
            "TimeWait",
            "Close",
            "CloseWait",
            "LastAck",
            "Listen",
            "Closing",
            "Unknown"
            ]
        }
        },
        {
        "name": "pid",
        "type": ["null", "int"],
        "default": null
        },
        {
        "name": "process_name",
        "type": ["null", "string"],
        "default": null
        },
        {
        "name": "tx_queue",
        "type": "int"
        },
        {
        "name": "rx_queue",
        "type": "int"
        }
    ]
    }
        "#
        }
        EvenType::UdpEvent(ref ev) => {
            r#"
{
  "type": "record",
  "name": "UdpEvent",
  "namespace": "watch-watch.network",
  "fields": [
    {
      "name": "timestamp",
      "type": "long"
    },
    {
      "name": "local_ip",
      "type": "string"
    },
    {
      "name": "local_port",
      "type": "int"
    },
    {
      "name": "pid",
      "type": ["null", "int"],
      "default": null
    },
    {
      "name": "process_name",
      "type": ["null", "string"],
      "default": null
    }
  ]
}
"#
        }
    };
    let schema = avro_rs::Schema::parse_str(raw_schema).unwrap();
    let mut writer = Writer::new(&schema, Vec::new());
    match ev_type {
        EvenType::TcpEvent(ev) => {
            let s = writer.append_ser(ev)?;
            let input = writer.into_inner()?;
            return Ok(input);
        }
        EvenType::UdpEvent(ev) => {
            let s = writer.append_ser(ev)?;
            let input = writer.into_inner()?;
            return Ok(input);
        }
    }
}
