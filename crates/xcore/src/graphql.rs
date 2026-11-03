//! Cliente GraphQL de x.com con sesión cookie (P1).
//!
//! Headers por petición (espejo del cliente web):
//! - `authorization`: bearer público de la web
//! - `x-csrf-token`: el `ct0` actual (también va como cookie `ct0=`)
//! - `cookie`: `auth_token=…; ct0=…`
//! - `user-agent`: Chrome estable; `x-twitter-active-user: yes`
//!
//! El `ct0` se ROTA ante `Set-Cookie` del server; el caller persiste la
//! `Session` devuelta por [`GqlClient::session`] (en la app, tras cada
//! llamada; en `xctl`, al término del comando).

use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Map, Value};

use crate::endpoints::{default_registry, OpKind, Registry};
use crate::errors::{Error, Result};
use crate::session::Session;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

pub struct GqlClient {
    http: reqwest::Client,
    bearer: String,
    registry: Registry,
    session: Mutex<Session>,
}

impl GqlClient {
    pub fn new(session: Session) -> Result<Self> {
        Ok(Self::with_registry(session, default_registry()?))
    }

    /// Registro alternativo (override externo del app dir / extractor P6).
    pub fn with_registry(session: Session, registry: Registry) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(UA)
            .timeout(Duration::from_secs(45))
            .build()
            .expect("reqwest client build");
        let bearer = registry.web_bearer.clone();
        Self {
            http,
            bearer,
            registry,
            session: Mutex::new(session),
        }
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    pub fn session(&self) -> Session {
        self.session.lock().unwrap().clone()
    }

    pub fn update_session(&self, s: Session) {
        *self.session.lock().unwrap() = s;
    }

    /// GraphQL GET (`variables`/`features`/`fieldToggles` en query string).
    pub async fn get(
        &self,
        op: &str,
        variables: Value,
        features_extra: Option<&Map<String, Value>>,
        field_toggles: Option<Map<String, Value>>,
    ) -> Result<Value> {
        let (url_s, kind) = self.registry.url_for(op)?;
        expect_kind(op, kind, OpKind::Get)?;

        let mut url = parse_url(&url_s, op)?;

        let mut params = Map::new();
        params.insert("variables".into(), variables);
        params.insert(
            "features".into(),
            Value::Object(self.registry.features_for(features_extra)),
        );
        if let Some(t) = field_toggles {
            params.insert("fieldToggles".into(), Value::Object(t));
        }
        // estilo twscrape `encode_params`: cada bloque es un query param con
        // JSON compacto (serde `Value::to_string()` ya es compacto); reqwest
        // hace el porcentaje-encoding.
        for (k, v) in &params {
            let s = match v {
                Value::String(s) => s.clone(),
                Value::Null => continue,
                other => other.to_string(),
            };
            url.query_pairs_mut().append_pair(k, &s);
        }

        self.request(op, url, None).await
    }

    /// GraphQL POST (JSON `{variables, features}` en body).
    pub async fn post(
        &self,
        op: &str,
        variables: Value,
        features_extra: Option<&Map<String, Value>>,
    ) -> Result<Value> {
        let (url_s, kind) = self.registry.url_for(op)?;
        expect_kind(op, kind, OpKind::Post)?;

        let mut params = Map::new();
        params.insert("variables".into(), variables);
        params.insert(
            "features".into(),
            Value::Object(self.registry.features_for(features_extra)),
        );

        self.request(op, parse_url(&url_s, op)?, Some(Value::Object(params)))
            .await
    }

    /// v1.1 fuera de GraphQL — devuelve el `screen_name` propio (`me`).
    pub async fn account_settings(&self) -> Result<Value> {
        let url = parse_url(
            "https://x.com/i/api/1.1/account/settings.json",
            "account_settings",
        )?;
        self.request("account_settings", url, None).await
    }

    async fn request(&self, op: &str, url: reqwest::Url, body: Option<Value>) -> Result<Value> {
        let s = self.session();
        let cookie = format!("auth_token={}; ct0={}", s.cookies.auth_token, s.cookies.ct0);

        let req = match body {
            None => self.http.get(url),
            Some(b) => self.http.post(url).json(&b),
        };

        let resp = req
            .header("authorization", &self.bearer)
            .header("x-csrf-token", &s.cookies.ct0)
            .header("cookie", &cookie)
            .header("x-twitter-active-user", "yes")
            .send()
            .await?;

        // rotación de ct0: persiste en el mutex (era el bug del draft).
        absorb_ct0(&resp, &self.session);

        let status = resp.status().as_u16();
        let retry_secs = resp
            .headers()
            .get("x-rate-limit-reset")
            .and_then(|h| h.to_str().ok())
            .and_then(|v| v.trim().parse::<i64>().ok())
            .map(|ts| (ts - now_secs()).max(0) as u64);

        let body = resp.text().await?;
        let val = serde_json::from_str::<Value>(&body)
            .unwrap_or_else(|_| json!({ "raw": body, "_http_status": status }));

        finish(status, retry_secs, &val, op)?;
        Ok(val)
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Parsea una URL del registro; cualquier falla es UnexpectedPayload (el
/// registro viene de datos embebidos/externos, no de redes).
fn parse_url(s: &str, op: &str) -> Result<reqwest::Url> {
    reqwest::Url::parse(s).map_err(|e| Error::UnexpectedPayload {
        op: op.into(),
        detail: format!("URL inválida: {e}"),
    })
}

/// Chequea que el `kind` del registro coincida con la llamada; evita
/// mezclar GET/POST por accidente.
fn expect_kind(op: &str, got: OpKind, want: OpKind) -> Result<()> {
    if got != want {
        return Err(Error::UnexpectedPayload {
            op: op.into(),
            detail: format!("la operación es {:?}, esperaba {:?}", got, want),
        });
    }
    Ok(())
}

/// Drena el nuevo `ct0` (si lo hay) de los Set-Cookie al mutex de sesión.
fn absorb_ct0(resp: &reqwest::Response, session: &Mutex<Session>) {
    let mut found = None;
    for h in resp.headers().get_all("set-cookie") {
        if let Ok(v) = h.to_str() {
            for piece in v.split(';') {
                if let Some((k, val)) = piece.trim().split_once('=') {
                    if k.eq_ignore_ascii_case("ct0") {
                        found = Some(val.to_string());
                    }
                }
            }
        }
    }
    if let Some(new_ct0) = found {
        session.lock().unwrap().rotate_ct0(new_ct0);
    }
}

/// Mapa HTTP/body → Ok(()) o Error accionable por la UI.
fn finish(status: u16, retry_secs: Option<u64>, body: &Value, op: &str) -> Result<()> {
    if let Some(errs) = body.get("errors").and_then(Value::as_array) {
        if let Some(first) = errs.first().and_then(Value::as_object) {
            let code = first.get("code").and_then(Value::as_i64).unwrap_or(0);
            let msg = first
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("sin mensaje");
            // 32/89/348: cookies vencidas o malas.
            if code == 32 || code == 89 || code == 348 {
                return Err(Error::SessionLost);
            }
            // 88/429: rate limit.
            if code == 88 || status == 429 {
                return Err(Error::RateLimited {
                    retry_after_secs: retry_secs.unwrap_or(60),
                });
            }
            return Err(Error::UnexpectedPayload {
                op: op.into(),
                detail: format!("X-GraphQL code {code}: {msg}"),
            });
        }
    }
    match status {
        200 | 201 => Ok(()),
        429 => Err(Error::RateLimited {
            retry_after_secs: retry_secs.unwrap_or(60),
        }),
        401 => Err(Error::SessionLost),
        403 => Err(Error::UnexpectedPayload {
            op: op.into(),
            detail: "403: prohibido (cuenta restringida o región)".into(),
        }),
        404 => Err(Error::EndpointStale { op: op.into() }),
        s => Err(Error::UnexpectedPayload {
            op: op.into(),
            detail: format!("HTTP {s}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;

    fn session() -> Session {
        Session::from_pasted(
            "auth_token=a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0; ct0=98f5c67f183f8765e1b96a4e5a3e1c4a",
        )
        .unwrap()
    }

    #[test]
    fn client_builds_with_registry() {
        let c = GqlClient::new(session()).unwrap();
        assert!(c.registry().url_for("UserByScreenName").is_ok());
        assert!(c.registry().web_bearer.starts_with("Bearer "));
        assert_eq!(c.session().cookies.ct0, "98f5c67f183f8765e1b96a4e5a3e1c4a");
    }

    /// Sin red: reproduce el armado de query de `get()` y verifica decodificando
    /// con `query_pairs` (decodifica URL encoding automáticamente).
    #[test]
    fn get_url_carries_params() {
        let c = GqlClient::new(session()).unwrap();
        let (url_s, kind) = c.registry().url_for("UserByScreenName").unwrap();
        assert_eq!(kind, OpKind::Get);

        let mut url = reqwest::Url::parse(&url_s).unwrap();
        let mut params = Map::new();
        params.insert("variables".into(), json!({"screen_name": "jack"}));
        params.insert(
            "features".into(),
            Value::Object(c.registry().features_for(None)),
        );
        for (k, v) in &params {
            let s = match v {
                Value::String(s) => s.clone(),
                Value::Null => continue,
                other => other.to_string(),
            };
            url.query_pairs_mut().append_pair(k, &s);
        }
        let pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
        assert_eq!(pairs.len(), 2);
        // serde_json Map ordena alfabético (features antes que variables) —
        // el orden en query string es irrelevante para el server; buscar por clave.
        let var = &pairs.iter().find(|p| p.0 == "variables").unwrap().1;
        assert!(var.contains(r#""screen_name":"jack""#));
        let feats = &pairs.iter().find(|p| p.0 == "features").unwrap().1;
        assert!(feats.contains(r#""view_counts_everywhere_api_enabled":true"#));
    }

    #[test]
    fn kind_guard() {
        assert!(expect_kind("A", OpKind::Get, OpKind::Get).is_ok());
        let e = expect_kind("A", OpKind::Get, OpKind::Post).unwrap_err();
        assert!(format!("{e}").contains("A"));
    }

    #[test]
    fn finish_maps_http_and_errors() {
        assert!(finish(200, None, &json!({}), "op").is_ok());
        assert!(matches!(
            finish(401, None, &json!({}), "op"),
            Err(Error::SessionLost)
        ));
        assert!(matches!(
            finish(404, None, &json!({}), "op"),
            Err(Error::EndpointStale { .. })
        ));
        assert!(matches!(
            finish(429, None, &json!({}), "op"),
            Err(Error::RateLimited {
                retry_after_secs: 60
            })
        ));
        assert!(matches!(
            finish(429, Some(7), &json!({}), "op"),
            Err(Error::RateLimited {
                retry_after_secs: 7
            })
        ));
        // errors[] en body con 200
        let b = json!({"errors": [{"code": 32, "message": "Bad authentication data"}]});
        assert!(matches!(
            finish(200, None, &b, "op"),
            Err(Error::SessionLost)
        ));
        let b = json!({"errors": [{"code": 88, "message": "Rate limit exceeded"}]});
        assert!(matches!(
            finish(200, Some(7), &b, "op"),
            Err(Error::RateLimited {
                retry_after_secs: 7
            })
        ));
        let b = json!({"errors": [{"code": 239, "message": "otra cosa"}]});
        assert!(matches!(
            finish(200, None, &b, "op"),
            Err(Error::UnexpectedPayload { .. })
        ));
    }
}
