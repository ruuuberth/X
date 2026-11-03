//! Sesión web de x.com por cookies pegadas (P1).
//!
//! Cookies críticas del cliente web:
//! - `auth_token` (HttpOnly, ~40 hex) — identidad persistente de la cuenta.
//! - `ct0` (CSRF; DEBE ir en header `x-csrf-token` y se ROTA por Set-Cookie en
//!   respuestas — hay que persistir el nuevo valor).
//!
//! La sesión se persiste cifrada "a futuro" (app dir privada + keystore Android
//! vía JNI en P6); localmente proseguimos JSON plano en el almacenamiento
//! privado de la app (fuera de alcance de otros apps por sandbox de Android).

use serde::{Deserialize, Serialize};

use crate::errors::{Error, Result};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cookies {
    #[serde(alias = "auth_token")]
    pub auth_token: String,
    #[serde(alias = "ct0")]
    pub ct0: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Session {
    pub handle: String,
    pub user_id: String,
    pub cookies: Cookies,
}

impl Session {
    /// Valida a partir de lo pegado por el usuario (JSON o pares `k=v`).
    pub fn from_pasted(raw: &str) -> Result<Session> {
        let c = parse_cookies(raw)?;
        validate(&c)?;
        Ok(Session {
            handle: String::new(),
            user_id: String::new(),
            cookies: c,
        })
    }

    /// Serializa para persistir (vault cifrado en P6).
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn from_json(json: &str) -> Result<Session> {
        let s: Session = serde_json::from_str(json)?;
        validate(&s.cookies)?;
        Ok(s)
    }

    /// Rota el ct0 cuando el servidor lo devuelve; invalida sesiones semivacías.
    pub fn rotate_ct0(&mut self, new_ct0: String) {
        if !new_ct0.is_empty() {
            self.cookies.ct0 = new_ct0;
        }
    }
}

/// Acepta `{"auth_token": "...", "ct0": "..."}` o pares `auth_token=...; ct0=...`
/// (formato de cabecera `Cookie:` pegada desde devtools).
fn parse_cookies(raw: &str) -> Result<Cookies> {
    let trimmed = raw.trim().trim_start_matches("Cookie:").trim();
    if trimmed.starts_with('{') {
        return serde_json::from_str::<Cookies>(trimmed).map_err(|e| Error::InvalidCookies {
            reason: format!("JSON no soportado: {e}"),
        });
    }
    let mut seek = Cookies::default();
    for piece in trimmed.split(';') {
        let piece = piece.trim();
        if let Some((k, v)) = piece.split_once('=') {
            let (k, v) = (k.trim(), v.trim());
            match k.to_ascii_lowercase().as_str() {
                "auth_token" => seek.auth_token = v.to_string(),
                "ct0" => seek.ct0 = v.to_string(),
                _ => {}
            }
        }
    }
    Ok(seek)
}

fn validate(c: &Cookies) -> Result<()> {
    let hex_ok = |s: &str| {
        s.len() >= 20 && s.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '%')
    };
    if !hex_ok(&c.auth_token) {
        return Err(Error::InvalidCookies {
            reason: "auth_token ausente o raro (esperado ~40 caracteres alfanuméricos)".into(),
        });
    }
    if !hex_ok(&c.ct0) {
        return Err(Error::InvalidCookies {
            reason: "ct0 ausente o raro (esperado ~32 caracteres alfanuméricos)".into(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW: &str =
        r#"auth_token=a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0; ct0=98f5c67f183f8765e1b96a4e5a3e1c4a"#;

    #[test]
    fn paste_cookie_header() {
        let s = Session::from_pasted(RAW).unwrap();
        assert_eq!(s.cookies.auth_token, "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0");
        assert_eq!(s.cookies.ct0, "98f5c67f183f8765e1b96a4e5a3e1c4a");
    }

    #[test]
    fn paste_raw_kv() {
        let s = Session::from_pasted("auth_token=abcdef0123456789abcdef0123456789abcdef01; ct0=0123456789abcdef0123456789abcdef").unwrap();
        assert_eq!(s.cookies.ct0, "0123456789abcdef0123456789abcdef");
    }

    #[test]
    fn paste_json() {
        let s = Session::from_pasted(r#"{"auth_token":"a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0","ct0":"98f5c67f183f8765e1b96a4e5a3e1c4a"}"#).unwrap();
        assert!(s.to_json().is_ok());
    }

    #[test]
    fn roundtrip_json() {
        let s = Session::from_pasted(RAW).unwrap();
        let parsed = Session::from_json(&s.to_json().unwrap()).unwrap();
        assert_eq!(parsed.cookies.auth_token, s.cookies.auth_token);
    }

    #[test]
    fn rejects_junk() {
        assert!(Session::from_pasted("no-ser-unauth").is_err());
        assert!(Session::from_pasted("auth_token=corto").is_err());
    }

    #[test]
    fn rotate() {
        let mut s = Session::from_pasted(RAW).unwrap();
        s.rotate_ct0("nuevoct0conlongitudsuficiente".into());
        assert_eq!(s.cookies.ct0, "nuevoct0conlongitudsuficiente");
    }
}
