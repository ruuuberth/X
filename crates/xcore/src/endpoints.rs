//! Registro de operaciones GraphQL de x.com.
//!
//! Los `queryId` rotan sin aviso. Este módulo carga el registro embebido
//! (`endpoints.json`) y permite override por archivo externo (app dir) y,
//! en P6, auto-descubrimiento scrapeando el bundle JS de la web.

use std::collections::HashMap;

use serde::Deserialize;

use crate::errors::{Error, Result};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OpKind {
    Get,
    Post,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Operation {
    pub kind: OpKind,
    /// URL con `%QUERY_ID%` placeholder.
    pub path: String,
    pub query_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Registry {
    pub schema: u8,
    pub ops: HashMap<String, Operation>,
    /// Mapa de features (se serializa como `variables.features`).
    pub features_union: serde_json::Map<String, serde_json::Value>,
}

/// Registro embebido (fallback si no hay archivo externo actualizado).
pub const DEFAULT_REGISTRY_JSON: &str = include_str!("../endpoints.json");

/// Carga el registro embebido.
pub fn default_registry() -> Result<Registry> {
    parse(DEFAULT_REGISTRY_JSON)
}

/// Carga un registro desde un archivo externo (app dir) si existe.
pub fn load_external(path: &std::path::Path) -> Option<Registry> {
    std::fs::read_to_string(path).ok().and_then(|s| parse(&s).ok())
}

pub fn parse(json: &str) -> Result<Registry> {
    let reg: Registry = serde_json::from_str(json).map_err(|e| {
        Error::UnexpectedPayload {
            op: "registry".into(),
            detail: format!("endpoints.json inválido: {e}"),
        }
    })?;
    if reg.schema != 1 {
        return Err(Error::UnexpectedPayload {
            op: "registry".into(),
            detail: format!("schema inesperado: {}", reg.schema),
        });
    }
    Ok(reg)
}

impl Registry {
    /// URL final del endpoint para una operación, con el queryId actual.
    pub fn url_for(&self, op: &str) -> Result<(String, OpKind)> {
        let o = self.ops.get(op).ok_or_else(|| Error::UnexpectedPayload {
            op: "registry".into(),
            detail: format!("operación desconocida: {op}"),
        })?;
        let qid = o
            .query_id
            .as_deref()
            .ok_or_else(|| Error::EndpointStale { op: op.into() })?;
        Ok((o.path.replace("%QUERY_ID%", qid), o.kind.clone()))
    }

    /// Marca/actualiza el queryId de una operación (usado por el extractor P6).
    pub fn set_query_id(&mut self, op: &str, qid: impl Into<String>) -> Result<()> {
        self.ops
            .get_mut(op)
            .ok_or_else(|| Error::UnexpectedPayload {
                op: "registry".into(),
                detail: format!("operación desconocida: {op}"),
            })?
            .query_id = Some(qid.into());
        Ok(())
    }
}

/// Extrae el queryId de una URL tipo `https://x.com/i/api/graphql/{id}/{Op?variables=...}`.
pub fn stash_query_id_from_url(url: &str) -> Option<(&str, &str)> {
    let rest = url.split("/i/api/graphql/").nth(1)?;
    let (qid, op) = rest.split_once('/')?;
    let op = op.split('?').next()?;
    Some((op, qid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_parses() {
        let reg = default_registry().unwrap();
        assert!(reg.ops.contains_key("Viewer"));
        assert_eq!(reg.ops["Viewer"].kind, OpKind::Get);
        // query_id sin asentar todavía → EndpointStale es el comportamiento esperado en P1 seed
        assert!(matches!(
            reg.url_for("Viewer"),
            Err(Error::EndpointStale { .. })
        ));
    }

    #[test]
    fn set_and_get_query_id() {
        let mut reg = default_registry().unwrap();
        reg.set_query_id("Viewer", "ABCD1234").unwrap();
        let (url, kind) = reg.url_for("Viewer").unwrap();
        assert!(url.contains("graphql/ABCD1234/Viewer"));
        assert_eq!(kind, OpKind::Get);
    }

    #[test]
    fn url_scraper() {
        let (op, qid) =
            stash_query_id_from_url("https://x.com/i/api/graphql/XyZ_9/HomeTimeline?variables=..")
                .unwrap();
        assert_eq!(op, "HomeTimeline");
        assert_eq!(qid, "XyZ_9");
    }
}
