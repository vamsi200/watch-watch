#![allow(unused)]
use crate::{
    config::{
        AgentId, CollectorConfig, CollectorKafkaConfig, CollectorProfile, Profile, ProfileId,
        TopicsConfig,
    },
    db::{
        fetch_collector_profile, fetch_profile, fetch_profile_id_by_token, fetch_profiles,
        update_agents, update_collector_profile, update_enrollment_tokens, update_profile,
    },
    server::{Register, ServerError},
};
use axum::{Json, extract::Query};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD_INDIFFERENT, prelude::*};
use chrono::{DateTime, Local, Utc};
use nanoid::alphabet::SAFE;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::error::Error;

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

#[derive(Debug)]
pub struct Agent {
    pub id: AgentId,           // uuid from collector
    pub profile_id: ProfileId, // profile it is using
    pub public_key: Vec<u8>,
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
    let collector_profile = fetch_collector_profile(&connection, profile_id)?;
    Ok(collector_profile)
}

pub fn encode_public_key(public_key: &[u8]) -> String {
    BASE64_STANDARD.encode(public_key)
}

pub fn decode_public_key(public_key: &str) -> anyhow::Result<Vec<u8>> {
    Ok(URL_SAFE_NO_PAD_INDIFFERENT.decode(public_key.as_bytes())?)
}

pub fn write_to_agent_db(public_key: String, token: &str, profile: &str) -> anyhow::Result<()> {
    let connection = Connection::open("/home/vamsi/.local/share/watch-watch/test.db").unwrap();
    let public_key = decode_public_key(&public_key)?;

    let agent = Agent {
        id: token.to_string(),
        profile_id: profile.to_string(),
        public_key,
        registered_at: Utc::now(),
        revoked: false,
    };

    update_agents(&connection, agent)?;

    Ok(())
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
    println!("[INFO] got request: {:?}", data);

    let (status, profile_id) = validate_token(&data.0.token);

    if status {
        let collector_profile = get_collector_profile(&profile_id);
        if let Err(_) = collector_profile {
            return Err(ServerError::Internal(anyhow::Error::msg(
                "Failed to register agent",
            )));
        }
        let collector_profile = collector_profile.unwrap();
        write_to_agent_db(data.0.public_key, &data.0.token, &profile_id).unwrap();
        return Ok(axum::Json::from(collector_profile));
    } else {
        return Err(ServerError::Unauthorized);
    }
}

pub fn get_agents() -> anyhow::Result<Vec<Agent>> {
    todo!()
}

pub fn get_agent(agent_id: AgentId) -> anyhow::Result<Agent> {
    todo!()
}

pub fn revoke_agent(agent_id: AgentId) -> anyhow::Result<()> {
    todo!()
}
