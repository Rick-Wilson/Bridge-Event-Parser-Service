use serde::Serialize;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub uptime_seconds: u64,
}

#[derive(Serialize)]
pub struct UploadResponse {
    pub session_id: String,
    /// Full enriched normalized JSON — client caches this in sessionStorage.
    pub normalized: serde_json::Value,
    pub event_name: Option<String>,
    pub event_date: Option<String>,
    pub players: Vec<String>,
    pub boards: Vec<u32>,
    pub board_count: usize,
    pub result_count: usize,
    pub has_pbn: bool,
    pub missing_names: usize,
    pub player_acbl: std::collections::HashMap<String, String>,
    pub missing_players: Vec<MissingPlayerInfo>,
    pub pair_acbl: std::collections::HashMap<String, Vec<Option<String>>>,
    pub sessions: Vec<SessionInfo>,
}

#[derive(Serialize)]
pub struct SessionInfo {
    pub session_idx: u32,
    pub label: String,
    pub board_count: usize,
    pub result_count: usize,
}

#[derive(Serialize)]
pub struct MissingPlayerInfo {
    pub display_name: String,
    pub acbl_number: Option<String>,
}
