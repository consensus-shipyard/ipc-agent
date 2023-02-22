use url::Url;
use crate::jsonrpc::JsonRpcClientImpl;
use crate::lotus::{LotusClient, LotusJsonRPCClient};

const HTTP_ENDPOINT: &str = "https://api.node.glif.io/rpc/v0";

fn get_lotus_client() -> LotusJsonRPCClient<JsonRpcClientImpl> {
    let url = Url::parse(HTTP_ENDPOINT).unwrap();
    let client = JsonRpcClientImpl::new(url, None);
    LotusJsonRPCClient::new(client)
}

#[tokio::test]
async fn state_network_name() {
    let client = get_lotus_client();
    assert_eq!(
        client.state_network_name().await.unwrap(),
        "mainnet"
    );
}

#[tokio::test]
async fn state_network_version() {
    let client = get_lotus_client();
    let version = client.state_network_version(vec![]).await.unwrap();

    // the version keeps on changing, as long as it's greater than 0, we
    // know it's returning some data.
    assert!(version > 0);
}