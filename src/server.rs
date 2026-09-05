#![allow(unused)]
use tokio::net::TcpListener;

use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use serde::Deserialize;
use serde_json::Value;

use crate::{config::AgentId, reg::register_agent};

pub enum ServerError {
    Unauthorized,
    Internal(anyhow::Error),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Unauthorized => {
                (StatusCode::UNAUTHORIZED, "authentication failed\n").into_response()
            }

            Self::Internal(err) => {
                eprintln!("internal server error: {err}");

                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error\n").into_response()
            }
        }
    }
}

#[derive(Debug)]
pub struct Server {
    pub addr: &'static str,
    pub port: u32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Register {
    pub token: String,
    pub agent_id: AgentId,
    pub public_key: String,
}

pub fn send_collector_config() -> anyhow::Result<()> {
    todo!()
}

// upon validation (validate_token()), register agent and send the config
// POST /enroll
// GET  /agents/:id
// POST /agents/:id/revoke
// GET  /agents/:id/config
pub async fn start_server(server: Server) -> anyhow::Result<()> {
    let addr = format!("{}:{}", server.addr, server.port);

    let route = Router::new().route("/enroll", get(register_agent));

    let listener = TcpListener::bind(&addr).await;

    if let Err(e) = listener {
        return Err(anyhow::bail!("{e}"));
    }

    let listener = listener?;

    println!("server started at: {addr}");

    axum::serve(listener, route.into_make_service())
        .await
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap();

    todo!()
}
