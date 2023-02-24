//! This module contains the various response types to be used byt the lotus api.

use std::str::FromStr;

use anyhow::anyhow;
use cid::Cid;
use serde::{Deserialize, Serialize};

pub(crate) mod chain;
pub(crate) mod mpool;
pub(crate) mod state;
pub(crate) mod wallet;

/// Exec actor parameters
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ExecParams {
    pub code_cid: Cid,
    pub constructor_params: Vec<u8>,
}

/// Install actor parameters
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct InstallActorParams {
    pub code: Vec<u8>,
}

/// Helper struct to interact with lotus node
#[derive(Deserialize, Serialize, Debug)]
pub struct CIDMap {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "/")]
    pub cid: Option<String>,
}

impl TryFrom<CIDMap> for Cid {
    type Error = anyhow::Error;

    fn try_from(cid_map: CIDMap) -> Result<Self, Self::Error> {
        let cid_option: Option<Cid> = cid_map.into();
        cid_option.ok_or(anyhow!("cid not found"))
    }
}

impl From<CIDMap> for Option<Cid> {
    fn from(m: CIDMap) -> Self {
        m.cid
            .map(|cid| Cid::from_str(&cid).expect("invalid cid str"))
    }
}

impl From<Option<Cid>> for CIDMap {
    fn from(c: Option<Cid>) -> Self {
        c.map(|cid| CIDMap {
            cid: Some(cid.to_string()),
        })
        .unwrap_or(CIDMap { cid: None })
    }
}

impl From<Cid> for CIDMap {
    fn from(c: Cid) -> Self {
        CIDMap {
            cid: Some(c.to_string()),
        }
    }
}
