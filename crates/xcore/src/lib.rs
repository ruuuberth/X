//! `xcore` — núcleo Rust de la app Android.
//!
//! Sin dependencias de Android: todo aquí es testeable en el host (`cargo test`).
//!
//! Mapa de módulos (P1):
//! - [`errors`]: taxonomía de errores accionable (sesión vencida, rate limit…)
//! - [`endpoints`]: registro de operaciones GraphQL (queryIds rotan; se auto-descubre en P6)
//! - [`models`]: modelos serde tolerantes (`#[serde(default)]`, aliases)
//! - [`session`]: sesión por cookies (`auth_token` + `ct0`) con persistencia
//!
//! Fase actual: **P1** — login por cookies, `me()`, `UserTweets`.

pub mod endpoints;
pub mod errors;
pub mod models;
pub mod session;

pub const APP_CODENAME: &str = "xclient";

/// Versión semántica del core (par con la app).
pub fn version() -> String {
    format!("{}+core", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_fmt() {
        assert!(version().starts_with("0.2.0"));
    }
}
