//! Errores de `xcore`: accionables por la UI (i18n es/en en P7).

use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// La sesión ya no es válida → la UI debe pedir nuevas cookies.
    SessionLost,
    /// El endpoint queryId está desactualizado → intentar re-descubrimiento.
    EndpointStale { op: String },
    /// 429 o similar → la UI debe esperar (segundos).
    RateLimited { retry_after_secs: u64 },
    /// Cookies pegadas malformadas (auth_token/ct0 ausentes o raras).
    InvalidCookies { reason: String },
    /// Payload JSON inesperado (X cambió algo) — degradación, no crash.
    UnexpectedPayload { op: String, detail: String },
    /// Error de transporte (mensaje plano; se re-tipa con reqwest en P1 client).
    Transport(String),
    Json(#[from] serde_json::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionLost => write!(f, "sesión perdida; pega cookies frescas"),
            Self::EndpointStale { op } => write!(f, "endpoint desactualizado: {op}"),
            Self::RateLimited { retry_after_secs } => {
                write!(f, "rate limit; reintenta en {retry_after_secs}s")
            }
            Self::InvalidCookies { reason } => write!(f, "cookies inválidas: {reason}"),
            Self::UnexpectedPayload { op, detail } => {
                write!(f, "payload inesperado en {op}: {detail}")
            }
            Self::Transport(m) => write!(f, "transporte: {m}"),
            Self::Json(e) => write!(f, "json: {e}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
