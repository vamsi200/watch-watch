#![allow(unused)]
use anyhow::{Context, anyhow};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::{
    config::{
        CollectorConfig, CollectorKafkaConfig, CollectorProfile, Profile, ProfileId, TopicsConfig,
    },
    reg::{Agent, EnrollmentToken},
};

pub fn create_tables(connection: &Connection) -> anyhow::Result<()> {
    println!("[INFO] Creating Tables...");

    connection.execute(
        "CREATE TABLE IF NOT EXISTS collector_profile (
            id  TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            config BLOB NOT NULL,
            version INTEGER NOT NULL DEFAULT 1,
            create_at INTEGER NOT NULL
        )",
        (),
    )?;

    connection.execute(
        "CREATE TABLE IF NOT EXISTS enrollment_tokens (
            id          BLOB PRIMARY KEY,
            token_hash  BLOB NOT NULL UNIQUE,
            profile_id  TEXT NOT NULL REFERENCES collector_profiles(id),
            created_at  INTEGER NOT NULL,
            expires_at  INTEGER,
            max_uses    INTEGER,
            uses        INTEGER NOT NULL DEFAULT 0,
            revoked     INTEGER NOT NULL DEFAULT 0
        )",
        (),
    )?;

    connection.execute(
        "CREATE TABLE IF NOT EXISTS agents (
            id           TEXT PRIMARY KEY,
            profile_id   TEXT NOT NULL REFERENCES collector_profiles(id),
            public_key   BLOB NOT NULL,
            registered_at INTEGER NOT NULL,
            revoked      INTEGER NOT NULL DEFAULT 0
        )",
        (),
    )?;

    connection.execute(
        "CREATE TABLE IF NOT EXISTS profiles (
            name TEXT NOT NULL UNIQUE,
            kafka_cluster TEXT NOT NULL,
            version INTEGER NOT NULL DEFAULT 1,
            kafka_config BLOB NOT NULL,
            config BLOB NOT NULL,
            agents_enrolled INTEGER NOT NULL DEFAULT 0
        )",
        (),
    )?;

    Ok(())
}

pub fn update_collector_profile(
    connection: &Connection,
    collector_profile: CollectorProfile,
    profile_id: &str,
    id: String,
) -> anyhow::Result<()> {
    let profile_bytes = serde_json::to_vec(&collector_profile.config)?;
    let now = Utc::now().timestamp();

    connection.execute(
        "INSERT INTO collector_profile (id, name, config, version, create_at) 
        VALUES (:id, :name, :config, :version, :create_at)",
        &[
            (":id", &id as &dyn rusqlite::ToSql),
            (":name", &profile_id as &dyn rusqlite::ToSql),
            (":config", &profile_bytes as &dyn rusqlite::ToSql),
            (
                ":version",
                &collector_profile.config.version as &dyn rusqlite::ToSql,
            ),
            (":create_at", &now as &dyn rusqlite::ToSql),
        ],
    )?;
    Ok(())
}

pub fn fetch_collector_profile(
    connection: &Connection,
    profile_id: &str,
) -> anyhow::Result<CollectorProfile> {
    let mut statement = connection.prepare(
        "SELECT name, config, version, create_at FROM collector_profile WHERE name = ?1",
    )?;

    let profile = statement.query_row([profile_id], |row| {
        let name: String = row.get(0)?;
        let config_blob: Vec<u8> = row.get(1)?;

        let config: CollectorConfig = serde_json::from_slice(&config_blob)
            .with_context(|| "failed to parse collector config blob")
            .unwrap();

        Ok(CollectorProfile { name, config })
    })?;

    Ok(profile)
}

pub fn update_enrollment_tokens(
    connection: &Connection,
    enrollment_token: EnrollmentToken,
) -> anyhow::Result<()> {
    let id_bytes = enrollment_token.id.as_bytes().to_vec();
    let mut hasher = Sha256::new();
    hasher.update(enrollment_token.token.as_bytes());
    let token_hash = hasher.finalize().to_vec();

    let created_at = enrollment_token.created_at.timestamp();
    let expires_at = enrollment_token.expires_at.timestamp();

    connection.execute(
        "INSERT INTO enrollment_tokens 
         (id, token_hash, profile_id, created_at, expires_at, max_uses, uses, revoked) 
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        (
            &id_bytes,
            &token_hash,
            &enrollment_token.profile_id,
            &created_at,
            &expires_at,
            &enrollment_token.max_uses,
            &enrollment_token.uses,
            &(enrollment_token.revoked as i64),
        ),
    )?;

    Ok(())
}

#[derive(thiserror::Error, Debug)]
pub enum TokenError {
    #[error("Enrollment token not found")]
    NotFound,
    #[error("Token has expired")]
    Expired,
    #[error("Token has been revoked")]
    Revoked,
    #[error("Token usage limit exceeded")]
    UsageExceeded,
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
}

pub fn fetch_profile_id_by_token(
    connection: &Connection,
    token: &str,
) -> Result<ProfileId, TokenError> {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let token_hash = hasher.finalize().to_vec();

    let mut statement = connection.prepare(
        "SELECT profile_id, expires_at, revoked, uses, max_uses 
         FROM enrollment_tokens WHERE token_hash = ?1",
    )?;

    let (profile_id, expires_at, revoked, uses, max_uses) = statement
        .query_row([&token_hash], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<i64>>(4)?,
            ))
        })
        .optional()?
        .ok_or(TokenError::NotFound)?;

    let now = Utc::now().timestamp();

    if expires_at < now {
        return Err(TokenError::Expired);
    }

    if revoked != 0 {
        return Err(TokenError::Revoked);
    }

    if let Some(max_uses) = max_uses {
        if uses >= max_uses {
            return Err(TokenError::UsageExceeded);
        }
    }

    connection.execute(
        "UPDATE enrollment_tokens SET uses = uses + 1 WHERE token_hash = ?1",
        [&token_hash],
    )?;

    Ok(profile_id)
}

// should be available for Admin Only
pub fn fetch_tokens(connection: &Connection) -> anyhow::Result<Vec<String>> {
    let mut statement = connection.prepare("SELECT token_hash FROM enrollment_tokens")?;

    let mut rows = statement.query([])?;
    let mut hashes = Vec::new();

    while let Some(row) = rows.next()? {
        let hash: Vec<u8> = row.get(0)?;
        let hash_hex = hex::encode(hash);
        hashes.push(hash_hex);
    }

    Ok(hashes)
}

pub fn update_agents(connection: &Connection, agent: Agent) -> anyhow::Result<()> {
    connection.execute(
        "INSERT INTO agents
            (id, profile_id, public_key, registered_at, revoked)
         VALUES
            (:id, :profile_id, :public_key, :registered_at, :revoked)",
        rusqlite::named_params! {
            ":id": agent.id,
            ":profile_id": agent.profile_id,
            ":public_key": agent.public_key,
            ":registered_at": agent.registered_at,
            ":revoked": agent.revoked,
        },
    )?;

    Ok(())
}

pub fn fecth_agent(connection: &Connection, agent_id: String) -> anyhow::Result<Agent> {
    let mut statement = connection
        .prepare("SELECT id, profile_id, public_key, registered_at, revoked WHERE id = ?1")?;

    let agent = statement.query_row([agent_id], |row| {
        let id: String = row.get(0)?;
        let profile_id: String = row.get(1)?;
        let public_key: Vec<u8> = row.get(2)?;
        let registered_at: DateTime<Utc> = row.get(3)?;
        let revoked: bool = row.get(4)?;

        Ok(Agent {
            id,
            profile_id,
            public_key,
            registered_at,
            revoked,
        })
    })?;

    Ok(agent)
}

pub fn fetch_agents(connection: &Connection) -> anyhow::Result<Vec<Agent>> {
    let mut statement = connection.prepare(
        "SELECT id, profile_id, public_key, registered_at, revoked
         FROM agents",
    )?;

    let agents = statement
        .query_map([], |row| {
            Ok(Agent {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                public_key: row.get(2)?,
                registered_at: row.get(3)?,
                revoked: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(agents)
}

pub fn update_profile(connection: &Connection, profile: Profile) -> anyhow::Result<()> {
    let config_bytes = serde_json::to_vec(&profile.config)?;
    let kafka_config_bytes = serde_json::to_vec(&profile.kafka_config)?;
    connection.execute(
        "INSERT INTO profiles (name, kafka_cluster, version, kafka_config, config, agents_enrolled) 
     VALUES (:name, :cluster, :version, :kafka_config, :config, :enrolled)",
        &[
            (":name", &profile.name as &dyn rusqlite::ToSql),
            (":cluster", &profile.kafka_cluster as &dyn rusqlite::ToSql),
            (":version", &profile.version as &dyn rusqlite::ToSql),
            (":kafka_config", &kafka_config_bytes as &dyn rusqlite::ToSql),
            (":config", &config_bytes as &dyn rusqlite::ToSql),
            (
                ":enrolled",
                &profile.agents_enrolled as &dyn rusqlite::ToSql,
            ),
        ],
    )?;

    Ok(())
}

pub fn fetch_profiles(connection: &Connection) -> anyhow::Result<Vec<Profile>> {
    let mut statement = connection.prepare(
        "SELECT name, kafka_cluster, version, kafka_config, config, agents_enrolled FROM profiles",
    )?;
    let mut rows = statement.query([])?;
    let mut profiles = Vec::new();

    while let Some(row) = rows.next()? {
        let name: String = row.get(0)?;
        let kafka_cluster: String = row.get(1)?;
        let version: i64 = row.get(2)?;
        let kafka_config_blob: Vec<u8> = row.get(3)?;
        let config_blob: Vec<u8> = row.get(4)?;
        let agents_enrolled: i64 = row.get(5)?;

        let kafka_config: CollectorKafkaConfig = serde_json::from_slice(&kafka_config_blob)?;
        let config: TopicsConfig = serde_json::from_slice(&config_blob)?;

        profiles.push(Profile {
            name,
            kafka_cluster,
            version,
            kafka_config,
            config,
            agents_enrolled,
        });
    }

    Ok(profiles)
}

pub fn fetch_profile(connection: &Connection, profile_name: &str) -> anyhow::Result<Profile> {
    let mut statement = connection.prepare(
        "SELECT name, kafka_cluster, config, agents_enrolled FROM profiles WHERE name = ?1",
    )?;

    let profile = statement.query_row([profile_name], |row| {
        let name: String = row.get(0)?;
        let kafka_cluster: String = row.get(1)?;
        let version: i64 = row.get(2)?;
        let kafka_config_blob: Vec<u8> = row.get(2)?;
        let config_blob: Vec<u8> = row.get(3)?;
        let agents_enrolled: i64 = row.get(4)?;

        let kafka_config: CollectorKafkaConfig = serde_json::from_slice(&kafka_config_blob)
            .with_context(|| "failed to parse CollectorKafkaConfig")
            .unwrap();

        let config: TopicsConfig = serde_json::from_slice(&config_blob)
            .with_context(|| "failed to parse config blob")
            .unwrap();

        Ok(Profile {
            name,
            kafka_cluster,
            version,
            kafka_config,
            config,
            agents_enrolled,
        })
    })?;

    Ok(profile)
}

pub fn table_exists(connection: &Connection, table_name: &str) -> anyhow::Result<bool> {
    let mut stmt =
        connection.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name = ?1")?;

    let exists = stmt
        .query_row([table_name], |row| row.get::<_, String>(0))
        .optional()?
        .is_some();

    Ok(exists)
}
