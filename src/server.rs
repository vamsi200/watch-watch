#![allow(unused)]
use serde_json::Value;

pub fn send_collector_config() -> anyhow::Result<()> {
    todo!()
}

// config here is CollectorProfile
// upon validation (validate_token()), register agent and send the config
// POST /enroll
// GET  /agents/:id
// POST /agents/:id/revoke
// GET  /agents/:id/config
pub fn start_server(config: Value) -> anyhow::Result<()> {
    todo!()
}
