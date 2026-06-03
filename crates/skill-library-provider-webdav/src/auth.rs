use skill_library_core::{AuthMode, ProviderCredential};

#[derive(Debug, Clone)]
pub enum WebDavAuth {
    Bearer(String),
    Basic { username: String, password: String },
}

impl WebDavAuth {
    pub(crate) fn from_credential(credential: &ProviderCredential) -> Option<Self> {
        let token = credential.token.trim();
        if token.is_empty() {
            return None;
        }
        match credential.metadata.auth_mode {
            AuthMode::Basic | AuthMode::AppPassword => {
                let (username, password) = match credential.metadata.login.as_deref() {
                    Some(login) if !login.trim().is_empty() => {
                        (login.trim().to_owned(), token.to_owned())
                    }
                    _ => token
                        .split_once(':')
                        .map(|(username, password)| (username.to_owned(), password.to_owned()))
                        .unwrap_or_else(|| (String::new(), token.to_owned())),
                };
                if username.is_empty() {
                    Some(Self::Bearer(token.to_owned()))
                } else {
                    Some(Self::Basic { username, password })
                }
            }
            _ => Some(Self::Bearer(token.to_owned())),
        }
    }
}
