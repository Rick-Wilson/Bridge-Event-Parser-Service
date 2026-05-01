use crate::responses::*;
use crate::upload_helpers;
use crate::AppState;
use axum::{
    extract::{ConnectInfo, Multipart, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

pub async fn healthz(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: state.version,
        uptime_seconds: state.started_at.elapsed().as_secs(),
    })
}

/// Upload BWS + optional PBN files. Parses, enriches, stores session,
/// and returns the full normalized JSON inline alongside metadata.
pub async fn upload_files(
    State(state): State<Arc<AppState>>,
    _headers: HeaderMap,
    _addr: ConnectInfo<SocketAddr>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, (StatusCode, String)> {
    let start = Instant::now();
    let tmp =
        tempfile::tempdir().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.file_name().unwrap_or("upload").to_string();
        let data = field
            .bytes()
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        std::fs::write(tmp.path().join(&name), &data)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    let mut bws_path = None;
    let mut pbn_path = None;
    for entry in std::fs::read_dir(tmp.path()).unwrap().flatten() {
        let path = entry.path();
        match path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str()
        {
            "bws" => bws_path = Some(path),
            "pbn" => pbn_path = Some(path),
            _ => {}
        }
    }
    let bws_path = bws_path.ok_or((StatusCode::BAD_REQUEST, "No BWS file uploaded".into()))?;

    let mut game = crate::adapters::pbn_bws::load_normalized(&bws_path, pbn_path.as_deref(), None)
        .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?;
    crate::builder::enrich_tricks(&mut game);
    crate::builder::enrich_handviewer_urls(&mut game);

    let session_id = uuid::Uuid::new_v4().to_string();
    state.sessions.insert(session_id.clone(), game.clone());

    tracing::info!(
        session_id = %session_id,
        elapsed_ms = %start.elapsed().as_millis(),
        "upload_files complete"
    );

    build_response(session_id, game, pbn_path.is_some())
}

/// Accept normalized JSON from the extension, validate, enrich, store, and
/// return the full normalized JSON inline so the SPA can cache it client-side.
pub async fn upload_normalized(
    State(state): State<Arc<AppState>>,
    _headers: HeaderMap,
    _addr: ConnectInfo<SocketAddr>,
    body: axum::body::Bytes,
) -> Result<Json<UploadResponse>, (StatusCode, String)> {
    let start = Instant::now();
    let body_str = std::str::from_utf8(&body)
        .map_err(|_| (StatusCode::BAD_REQUEST, "body is not valid UTF-8".into()))?;

    let mut game = crate::schema::parse_normalized(body_str).map_err(|e| {
        let code = match e {
            crate::schema::ParseError::UnsupportedMajor { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            _ => StatusCode::BAD_REQUEST,
        };
        (code, e.to_string())
    })?;
    crate::builder::enrich_tricks(&mut game);
    crate::builder::enrich_handviewer_urls(&mut game);

    if upload_helpers::flatten_sessions(&game).is_empty() {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "No sessions found".into()));
    }

    let session_id = uuid::Uuid::new_v4().to_string();
    state.sessions.insert(session_id.clone(), game.clone());

    tracing::info!(
        session_id = %session_id,
        elapsed_ms = %start.elapsed().as_millis(),
        "upload_normalized complete"
    );

    build_response(session_id, game, true)
}

/// Retrieve a stored session's normalized JSON (used by the batch flow).
pub async fn get_normalized(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let session_id = params.get("session").ok_or((
        StatusCode::BAD_REQUEST,
        "Missing 'session' parameter".into(),
    ))?;
    let game = state
        .sessions
        .get(session_id)
        .ok_or((StatusCode::NOT_FOUND, "Session not found or expired".into()))?;
    let body = serde_json::to_string(&game)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body,
    ))
}

// ==================== Helpers ====================

fn build_response(
    session_id: String,
    game: crate::schema::NormalizedGame,
    has_pbn: bool,
) -> Result<Json<UploadResponse>, (StatusCode, String)> {
    let normalized = serde_json::to_value(&game)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let flat = upload_helpers::flatten_sessions(&game);
    let first = flat.first().ok_or((
        StatusCode::UNPROCESSABLE_ENTITY,
        "No sessions in upload".into(),
    ))?;

    let session_infos: Vec<SessionInfo> = flat
        .iter()
        .map(|s| SessionInfo {
            session_idx: s.session_idx,
            label: s.label.clone(),
            board_count: s.session.boards.len(),
            result_count: upload_helpers::result_count(s.session),
        })
        .collect();

    let summary = upload_helpers::summarize_players(first.session);
    let boards = upload_helpers::board_numbers(first.session);
    let result_count = upload_helpers::result_count(first.session);
    let event_date = first.event_date.as_deref().map(reformat_event_date);

    Ok(Json(UploadResponse {
        session_id,
        normalized,
        event_name: first.event_name.clone(),
        event_date,
        players: summary.display_names,
        board_count: boards.len(),
        boards,
        result_count,
        has_pbn,
        missing_names: summary.missing_players.len(),
        player_acbl: summary.player_acbl,
        missing_players: summary.missing_players,
        pair_acbl: summary.pair_acbl,
        sessions: session_infos,
    }))
}

/// BWS dates look like "03/30/26 00:00:00" — reformat as YYYY-MM-DD.
fn reformat_event_date(d: &str) -> String {
    let date_part = d.split(' ').next().unwrap_or(d);
    let parts: Vec<&str> = date_part.split('/').collect();
    if parts.len() == 3 {
        let year = if parts[2].len() == 2 {
            format!("20{}", parts[2])
        } else {
            parts[2].to_string()
        };
        format!("{}-{}-{}", year, parts[0], parts[1])
    } else {
        date_part.to_string()
    }
}
