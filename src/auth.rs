use rand::{distributions::Alphanumeric, Rng};
use std::sync::Arc;

#[derive(Clone)]
pub struct AuthState {
    pub token: Arc<String>,
}

impl AuthState {
    pub fn new() -> Self {
        let token: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();

        Self {
            token: Arc::new(token),
        }
    }

    pub fn is_valid(&self, token: &str) -> bool {
        self.token.as_str() == token
    }
}
