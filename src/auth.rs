use rand::{distributions::Alphanumeric, Rng};
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct AuthState {
    pub token: Arc<RwLock<String>>,
}

impl AuthState {
    pub fn new() -> Self {
        let token: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();

        Self::with_token(token)
    }

    pub fn with_token(token: String) -> Self {
        Self {
            token: Arc::new(RwLock::new(token)),
        }
    }

    pub fn get_token(&self) -> String {
        self.token.read().unwrap().clone()
    }

    pub fn set_token(&self, new_token: String) {
        let mut t = self.token.write().unwrap();
        *t = new_token;
    }

    pub fn is_valid(&self, token: &str) -> bool {
        self.token.read().unwrap().as_str() == token
    }
}
