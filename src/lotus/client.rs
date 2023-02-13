use crate::response::{
    CIDMap, MpoolPushMessage, MpoolPushMessageInner, MpoolPushMessageResponse, ReadStateResponse,
    StateWaitMsgResponse, WalletKeyType, WalletListResponse,
};
use crate::JsonRpcClient;
use crate::LotusApi;
use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use num_traits::cast::ToPrimitive;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::fmt::Debug;
use std::str::FromStr;

// RPC methods
mod methods {
    pub const MPOOL_PUSH_MESSAGE: &str = "Filecoin.MpoolPushMessage";
    pub const STATE_WAIT_MSG: &str = "Filecoin.StateWaitMsg";
    pub const WALLET_NEW: &str = "Filecoin.WalletNew";
    pub const WALLET_LIST: &str = "Filecoin.WalletList";
    pub const WALLET_DEFAULT_ADDRESS: &str = "Filecoin.WalletDefaultAddress";
    pub const STATE_READ_STATE: &str = "Filecoin.StateReadState";
}

/// The lotus client that provides the basic Lotus Node API abstraction.
/// Only basic functions are provided.
pub struct LotusClient<Inner> {
    inner: Inner,
}

impl<Inner> LotusClient<Inner> {
    pub fn new(inner: Inner) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl<Inner: JsonRpcClient + Send + Sync> LotusApi for LotusClient<Inner> {
    async fn mpool_push_message(&self, msg: MpoolPushMessage) -> Result<MpoolPushMessageInner> {
        let from = if let Some(f) = msg.from {
            f
        } else {
            self.wallet_default().await?
        };

        let nonce = msg
            .nonce
            .map(|n| serde_json::Value::Number(n.into()))
            .unwrap_or(serde_json::Value::Null);

        let to_value = |t: Option<TokenAmount>| {
            t.map(|n| serde_json::Value::Number(n.atto().to_u64().unwrap().into()))
                .unwrap_or(serde_json::Value::Null)
        };
        let gas_limit = to_value(msg.gas_limit);
        let gas_premium = to_value(msg.gas_premium);
        let gas_fee_cap = to_value(msg.gas_fee_cap);
        let max_fee = to_value(msg.max_fee);

        // refer to: https://lotus.filecoin.io/reference/lotus/mpool/#mpoolpushmessage
        let to_send = json!([
            {
                "to": msg.to.to_string(),
                "from": from.to_string(),
                "value": msg.value.atto().to_string(),
                "method": msg.method,
                "params": msg.params,

                // THESE ALL WILL AUTO POPULATE if null
                "nonce": nonce,
                "gas_limit": gas_limit,
                "gas_fee_cap": gas_fee_cap,
                "gas_premium": gas_premium,
                "cid": CIDMap::from(msg.cid),
                "version": serde_json::Value::Null,
            },
            {
                "max_fee": max_fee
            }
        ]);

        let r = self
            .inner
            .request::<MpoolPushMessageResponse>(methods::MPOOL_PUSH_MESSAGE, to_send)
            .await?;
        log::debug!("received mpool_push_message response: {r:?}");

        Ok(r.message)
    }

    async fn state_wait_msg(&self, cid: Cid, nonce: u64) -> Result<StateWaitMsgResponse> {
        // refer to: https://lotus.filecoin.io/reference/lotus/state/#statewaitmsg
        let to_send = json!([CIDMap::from(cid), nonce]);

        let r = self
            .inner
            .request::<StateWaitMsgResponse>(methods::STATE_WAIT_MSG, to_send)
            .await?;
        log::debug!("received state_wait_msg response: {r:?}");
        Ok(r)
    }

    async fn wallet_default(&self) -> Result<Address> {
        // refer to: https://lotus.filecoin.io/reference/lotus/wallet/#walletdefaultaddress
        let r = self
            .inner
            .request::<String>(methods::WALLET_DEFAULT_ADDRESS, json!({}))
            .await?;
        log::debug!("received wallet_default response: {r:?}");

        let addr = Address::from_str(&r)?;
        Ok(addr)
    }

    async fn wallet_list(&self) -> Result<WalletListResponse> {
        // refer to: https://lotus.filecoin.io/reference/lotus/wallet/#walletlist
        let r = self
            .inner
            .request::<WalletListResponse>(methods::WALLET_LIST, json!({}))
            .await?;
        log::debug!("received wallet_list response: {r:?}");
        Ok(r)
    }

    async fn wallet_new(&self, key_type: WalletKeyType) -> Result<String> {
        let s = key_type.as_ref();
        // refer to: https://lotus.filecoin.io/reference/lotus/wallet/#walletnew
        let r = self
            .inner
            .request::<String>(methods::WALLET_NEW, json!([s]))
            .await?;
        log::debug!("received wallet_new response: {r:?}");
        Ok(r)
    }

    async fn read_state<State: DeserializeOwned + Debug>(
        &self,
        address: Address,
        tipset: Cid,
    ) -> Result<ReadStateResponse<State>> {
        // refer to: https://lotus.filecoin.io/reference/lotus/state/#statereadstate
        let r = self
            .inner
            .request::<ReadStateResponse<State>>(
                methods::STATE_READ_STATE,
                json!([address.to_string(), [CIDMap::from(tipset)]]),
            )
            .await?;
        log::debug!("received read_state response: {r:?}");
        Ok(r)
    }
}
