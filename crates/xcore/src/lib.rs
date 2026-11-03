//! `xcore` — núcleo Rust de la app Android.
//!
//! Sin dependencias de Android: todo aquí es testeable en el host
//! (`cargo test`). Construimos en P1: sesión por cookies, cliente
//! GraphQL de x.com, timeline, acciones y media.
//!
//! Fases: P1 login/cookies → P2 feed → P3 acciones → P4 upload → P5 download.

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
        assert!(version().starts_with("0.1.0"));
    }
}
