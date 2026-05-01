use crate::schema::NormalizedGame;
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

const TTL: Duration = Duration::from_secs(3600);

struct Entry {
    game: NormalizedGame,
    stored_at: Instant,
}

pub struct SessionStore {
    inner: RwLock<HashMap<String, Entry>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    pub fn insert(&self, id: String, game: NormalizedGame) {
        self.inner.write().unwrap().insert(
            id,
            Entry {
                game,
                stored_at: Instant::now(),
            },
        );
    }

    pub fn get(&self, id: &str) -> Option<NormalizedGame> {
        let map = self.inner.read().unwrap();
        let entry = map.get(id)?;
        if entry.stored_at.elapsed() > TTL {
            return None;
        }
        Some(entry.game.clone())
    }

    pub fn sweep(&self) {
        self.inner
            .write()
            .unwrap()
            .retain(|_, e| e.stored_at.elapsed() <= TTL);
    }
}
