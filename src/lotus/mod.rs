pub mod client;
///! The lotus api to interact with lotus node
pub mod response;

use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use fvm_shared::address::Address;
use serde::de::DeserializeOwned;
use std::fmt::Debug;

pub use crate::lotus::client::LotusClient;
pub use crate::lotus::response::{MpoolPushMessage, MpoolPushMessageInner};
pub use crate::lotus::response::{
    ReadStateResponse, StateWaitMsgResponse, WalletKeyType, WalletListResponse,
};

#[async_trait]
pub trait LotusApi {
    /// Push the message to memory pool, see: https://lotus.filecoin.io/reference/lotus/mpool/#mpoolpushmessage
    async fn mpool_push_message(&self, msg: MpoolPushMessage) -> Result<MpoolPushMessageInner>;

    /// Wait for the message cid of a particular nonce, see: https://lotus.filecoin.io/reference/lotus/state/#statewaitmsg
    async fn state_wait_msg(&self, cid: Cid, nonce: u64) -> Result<StateWaitMsgResponse>;

    /// Get the default wallet of the node, see: https://lotus.filecoin.io/reference/lotus/wallet/#walletdefaultaddress
    async fn wallet_default(&self) -> Result<Address>;

    /// List the wallets in the node, see: https://lotus.filecoin.io/reference/lotus/wallet/#walletlist
    async fn wallet_list(&self) -> Result<WalletListResponse>;

    /// Create a new wallet, see: https://lotus.filecoin.io/reference/lotus/wallet/#walletnew
    async fn wallet_new(&self, key_type: WalletKeyType) -> Result<String>;

    /// Read the state of the address at tipset, see: https://lotus.filecoin.io/reference/lotus/state/#statereadstate
    async fn read_state<State: DeserializeOwned + Debug>(
        &self,
        address: Address,
        tipset: Cid,
    ) -> Result<ReadStateResponse<State>>;
}
