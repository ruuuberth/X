//! Errores de `xcore`: accionables por la UI (i18n es/en en P7).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// La sesión ya no es válida → la UI debe pedir nuevas cookies.
    #[error("sesión perdida; pega cookies frescas")]
    SessionLost,
    /// El endpoint queryId está desactualizado → intentar re-descubrimiento.
    #[error("endpoint desactualizado: {op}")]
    EndpointStale { op: String },
    /// 429 o similar → la UI debe esperar (segundos).
    #[error("rate limit; reintenta en {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },
    /// Cookies pegadas malformadas (auth_token/ct0 ausentes o raras).
    #[error("cookies inválidas: {reason}")]
    InvalidCookies { reason: String },
    /// Payload JSON inesperado (X cambió algo) — degradación, no crash.
    #[error("payload inesperado en {op}: {detail}")]
    UnexpectedPayload { op: String, detail: String },
    /// Error de transporte (DNS, TLS, timeout…).
    #[error("transporte: {0}")]
    Transport(#[from] reqwest::Error),
    /// Serialización/parseo de JSON.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
