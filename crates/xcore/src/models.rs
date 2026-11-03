//! Modelos serde tolerantes (el esquema GraphQL de X cambia; `defaults` y
//! aliases evitan crashes y degradamos a "payload inesperado" donde importa).
//!
//! Dos formas de payload coexisten en la API viva:
//! - **nueva (2026)**: `core.screen_name`, `avatar.image_url`,
//!   `relationship_counts.followers`, `profile_bio.description`…
//! - **legacy**: `legacy.screen_name`, `legacy.profile_image_url_https`,
//!   `legacy.followers_count`…
//!
//! El mapeador ([`crate::mapper`]) contempla ambas; aquí solo viven los
//! tipos que comparte la UI.

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
    #[serde(default)]
    pub bio: String,
    #[serde(default)]
    pub followers: u64,
    #[serde(default)]
    pub following: u64,
    #[serde(default)]
    pub statuses: u64,
    /// Fecha original de la API (formato X: `Sat Oct 17 18:11:27 +0000 2026`).
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub is_blue_verified: bool,
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
    /// Foto: `media_url_https` base. Video/Gif: variante mp4 de mayor bitrate.
    pub url: String,
    /// Solo video/gif: duración en ms (0 si desconocida).
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub struct Tweet {
    pub id: String,
    pub author: User,
    pub text: String,
    pub created_at_ms: i64,
    /// Padre si es una respuesta (`in_reply_to_status_id_str` si está).
    pub reply_to_id: Option<String>,
    pub media: Vec<Media>,
    pub liked: bool,
    pub reposted: bool,
    pub like_count: u64,
    pub repost_count: u64,
    pub reply_count: u64,
    /// La payload era un RT (contenido enriquecido del tweet interno).
    pub is_retweet: bool,
    /// URL propia del tweet (`https://x.com/{screen}/status/{id}`) para abrir.
    pub permalink: String,
}

impl Tweet {
    /// Texto "con enlaces embebidos separados" (para la UI).
    // TODO(P1): enlaces t.co→url expandida y #menciones resaltables.
    pub fn plain_text(&self) -> &str {
        &self.text
    }
}
