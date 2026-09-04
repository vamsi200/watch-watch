#![allow(unused)]
use std::error::Error;

use crate::{
    config::{
        AgentId, CollectorConfig, CollectorKafkaConfig, CollectorProfile, Profile, ProfileId,
        TopicsConfig,
    },
    db::{
        fetch_collector_profile, fetch_profile, fetch_profile_id_by_token, fetch_profiles,
        update_collector_profile, update_enrollment_tokens, update_profile,
    },
    server::{Register, ServerError},
};
use axum::{Json, extract::Query};
use chrono::{DateTime, Local, Utc};
use nanoid::alphabet::SAFE;
use rusqlite::Connection;
use serde_json::{Value, json};

pub struct EnrollmentToken {
    pub id: String,
    pub token: String,
    pub profile_id: ProfileId,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub max_uses: i64,
    pub uses: i64,
    pub revoked: bool,
}

pub struct Agent {
    pub id: AgentId,
    pub profile_id: ProfileId,
    pub registered_at: DateTime<Utc>,
    pub revoked: bool,
}

pub fn create_enrollment_token(
    connection: &Connection,
    profile_id: ProfileId,
    expiration: DateTime<Utc>,
    max_uses: i64,
) -> anyhow::Result<(String, String)> {
    let dt = Local::now();
    let created_at = dt.to_utc();
    let token = nanoid::nanoid!(32, &SAFE);
    let id = nanoid::nanoid!(16, &SAFE);

    let enrollment_token = EnrollmentToken {
        id: id.clone(),
        token: token.clone(),
        profile_id,
        expires_at: expiration,
        created_at,
        max_uses,
        uses: 0,
        revoked: false,
    };

    update_enrollment_tokens(&connection, enrollment_token)?;

    Ok((token, id))
}

pub fn create_collector_profile(
    connection: &Connection,
    collector_profile: CollectorProfile,
    profile_id: ProfileId,
    id: String,
) -> anyhow::Result<()> {
    update_collector_profile(&connection, collector_profile, &profile_id, id)?;
    Ok(())
}

pub fn create_profile(connection: &Connection, profile: Profile) -> anyhow::Result<()> {
    update_profile(&connection, profile)?;
    Ok(())
}

pub fn get_profile(connection: Connection, profile_id: ProfileId) -> anyhow::Result<Profile> {
    Ok(fetch_profile(&connection, &profile_id)?)
}

pub fn get_profiles(connection: Connection) -> anyhow::Result<Vec<Profile>> {
    Ok(fetch_profiles(&connection)?)
}

pub fn get_collector_profile(profile_id: &str) -> anyhow::Result<CollectorProfile> {
    //TODO: change this
    let connection = Connection::open("/home/vamsi/.local/share/watch-watch/test.db").unwrap();
    Ok(fetch_collector_profile(&connection, profile_id)?)
}

pub fn validate_token(token: &String) -> (bool, String) {
    //TODO: change this
    let connection = Connection::open("/home/vamsi/.local/share/watch-watch/test.db").unwrap();

    match fetch_profile_id_by_token(&connection, token) {
        Ok(profile_id) => (true, profile_id),
        Err(e) => {
            eprintln!("{e}");
            (false, String::new())
        }
    }
}

pub async fn register_agent(data: Query<Register>) -> Result<Json<CollectorProfile>, ServerError> {
    let (status, profile_id) = validate_token(&data.0.token);

    if status {
        let collector_profile = get_collector_profile(&profile_id);
        if let Err(_) = collector_profile {
            return Err(ServerError::Internal(anyhow::Error::msg(
                "Failed to register agent",
            )));
        }
        let collector_profile = collector_profile.unwrap();
        return Ok(axum::Json::from(collector_profile));
    } else {
        return Err(ServerError::Unauthorized);
    }
}

pub fn get_agent(agent_id: AgentId) -> anyhow::Result<Agent> {
    todo!()
}

pub fn revoke_agent(agent_id: AgentId) -> anyhow::Result<()> {
    todo!()
}
