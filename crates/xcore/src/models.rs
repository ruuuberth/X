//! Modelos serde tolerantes (el esquema GraphQL de X cambia; `defaults` y
//! aliases evitan crashes y degradamos a "payload inesperado" donde importa).
//!
//! Se completan con extractores (`map_*`) cuando el cliente real (P1) devuelva
//! payloads vivos; por ahora son los tipos núcleo compartidos por UI.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct User {
    #[serde(default)]
    pub id: String,
    #[serde(default, alias = "screenName")]
    pub screen_name: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, alias = "profileImageUrlHttps")]
    pub avatar_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Photo,
    Video,
    Gif,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
    pub kind: MediaKind,
    pub media_id: String,
    pub url: String,
}

#[derive(Debug, Clone, Default)]
pub struct Tweet {
    pub id: String,
    pub author: User,
    pub text: String,
    pub created_at_ms: i64,
    /// Padre si es una respuesta.
    pub reply_to_id: Option<String>,
    pub media: Vec<Media>,
    pub liked: bool,
    pub reposted: bool,
    pub like_count: u64,
    pub repost_count: u64,
    pub reply_count: u64,
}

impl Tweet {
    /// Texto "con enlaces embebidos separados" (para la UI).
    // TODO(P1): enlaces t.co→url expandida y #mentions resaltables.
    pub fn plain_text(&self) -> &str {
        &self.text
    }
}
