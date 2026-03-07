#![allow(unused)]

use watch_watch::parser::{self, serialize_data};

fn main() -> anyhow::Result<()> {
    let tcp_event = parser::parse_proc_net_tcp()?;
    for ev in tcp_event {
        serialize_data(ev)?;
    }
    // let udp_event = parser::parse_net_udp()?;
    // println!("{udp_event:#?}");
    Ok(())
}
