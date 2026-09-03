#![allow(unused)]
use crate::config::{AgentId, ProfileId};
use chrono::{DateTime, Utc};

pub struct EnrollmentToken {
    pub token: String,
    pub profile_id: ProfileId,
    pub expires_at: DateTime<Utc>,
    pub max_uses: u32,
    pub uses: u32,
}

pub struct Agent {
    pub id: AgentId,
    pub profile_id: ProfileId,
    pub registered_at: DateTime<Utc>,
    pub revoked: bool,
}

pub fn create_enrollment_token(
    profile_id: ProfileId,
    expiration: DateTime<Utc>,
    max_uses: u32,
) -> anyhow::Result<String> {
    todo!()
}

pub fn register_agent(token: &str, agent_id: AgentId) -> anyhow::Result<Agent> {
    todo!()
}

pub fn get_agent(agent_id: AgentId) -> anyhow::Result<Agent> {
    todo!()
}

pub fn revoke_agent(agent_id: AgentId) -> anyhow::Result<()> {
    todo!()
}

pub fn validate_token() -> anyhow::Result<()> {
    todo!()
}
