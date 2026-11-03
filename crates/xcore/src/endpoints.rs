//! Registro de operaciones GraphQL de x.com (esquema 2).
//!
//! Los `queryId` rotan sin aviso: DEFENSAS
//! 1. `query_id` sembrados verificados en el día (origen público twscrape-update).
//! 2. P6: extractor del JS bundle de x.com que refresca el registro en caliente
//!    (override externo `endpoints.json` en el app dir > embebido).

use std::collections::HashMap;

use serde::Deserialize;

use crate::errors::{Error, Result};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OpKind {
    Get,
    Post,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Operation {
    pub kind: OpKind,
    pub query_id: String,
}

#[derive(Debug, Deserialize)]
pub struct Registry {
    pub schema: u8,
    /// Bearer público del cliente web (no es secreto; va en `authorization`).
    pub web_bearer: String,
    #[serde(default = "default_base")]
    pub graphql_base: String,
    pub ops: HashMap<String, Operation>,
    /// features comunes (se fusiona con extras por operación).
    pub features_base: serde_json::Map<String, serde_json::Value>,
}

fn default_base() -> String {
    "https://x.com/i/api/graphql".into()
}

pub const DEFAULT_REGISTRY_JSON: &str = include_str!("../endpoints.json");

pub fn default_registry() -> Result<Registry> {
    parse(DEFAULT_REGISTRY_JSON)
}

pub fn load_external(path: &std::path::Path) -> Option<Registry> {
    std::fs::read_to_string(path)
        .map(|s| parse(&s).ok())
        .ok()
        .flatten()
}

pub fn parse(json: &str) -> Result<Registry> {
    let reg: Registry = serde_json::from_str(json).map_err(|e| Error::UnexpectedPayload {
        op: "registry".into(),
        detail: format!("endpoints.json inválido: {e}"),
    })?;
    if reg.schema < 2 {
        return Err(Error::UnexpectedPayload {
            op: "registry".into(),
            detail: format!("schema viejo: {}", reg.schema),
        });
    }
    Ok(reg)
}

impl Registry {
    /// URL `{base}/{query_id}/{op}` para una operación.
    pub fn url_for(&self, op: &str) -> Result<(String, OpKind)> {
        let o = self.ops.get(op).ok_or_else(|| Error::UnexpectedPayload {
            op: "registry".into(),
            detail: format!("operación desconocida: {op}"),
        })?;
        let url = format!("{}/{}/{}", self.graphql_base, o.query_id, op);
        Ok((url, o.kind))
    }

    /// Fusiona `features_base` con extras propios de la operación.
    pub fn features_for(
        &self,
        extras: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut all = self.features_base.clone();
        if let Some(x) = extras {
            for (k, v) in x.iter() {
                all.insert(k.clone(), v.clone());
            }
        }
        all
    }

    pub fn set_query_id(&mut self, op: &str, qid: impl Into<String>) -> Result<()> {
        self.ops
            .get_mut(op)
            .ok_or_else(|| Error::UnexpectedPayload {
                op: "registry".into(),
                detail: format!("operación desconocida: {op}"),
            })?
            .query_id = qid.into();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_new_schema() {
        let reg = default_registry().unwrap();
        assert_eq!(reg.schema, 2);
        assert!(reg.web_bearer.starts_with("Bearer "));
        let (url, kind) = reg.url_for("UserByScreenName").unwrap();
        assert!(url.ends_with("UserByScreenName"));
        assert!(url.contains("AMIBMjtxEEATh4z8V9GtRg"));
        assert_eq!(kind, OpKind::Get);
    }

    #[test]
    fn features_merge() {
        let reg = default_registry().unwrap();
        let mut extra = serde_json::Map::new();
        extra.insert("hidden_profile_likes_enabled".into(), true.into());
        let ft = reg.features_for(Some(&extra));
        assert_eq!(
            ft.get("view_counts_everywhere_api_enabled"),
            Some(&true.into())
        );
        assert_eq!(ft.get("hidden_profile_likes_enabled"), Some(&true.into()));
    }
}
