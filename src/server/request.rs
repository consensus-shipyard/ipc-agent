use serde_json::Value;
use serde::{Deserialize, Serialize};

/// The json rpc request param. It is the standard form our json-rpc and follows a structure similar
/// to the one of the Ethereum RPC: https://ethereum.org/en/developers/docs/apis/json-rpc/#curl-examples
#[derive(Serialize, Deserialize, Debug)]
pub struct JSONRPCRequest {
    pub id: u16,
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
}
