#![allow(unused)]

use watch_watch::parser::{self, connect_kafka, serialize_data};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let tcp_events = parser::parse_proc_net_tcp()?;
    let udp_events = parser::parse_net_udp()?;

    println!("No of tcp events being sent {}", tcp_events.len());
    println!("No of udp events being sent {}", udp_events.len());

    for ev in tcp_events {
        let key = ev.local_ip.clone();
        let data = serialize_data(parser::EvenType::TcpEvent(ev))?;
        connect_kafka(data, "tcp.events", &key).await?;
    }

    for ev in udp_events {
        let key = ev.local_ip.clone();
        let data = serialize_data(parser::EvenType::UdpEvent(ev))?;
        connect_kafka(data, "udp.events", &key).await?;
    }

    Ok(())
}
