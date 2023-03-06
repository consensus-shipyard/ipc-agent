// Copyright 2022-2023 Protocol Labs
// SPDX-License-Identifier: MIT
//! The shared subnet manager module for all subnet management related RPC method calls.

use crate::config::{ReloadableConfig, Subnet};
use crate::jsonrpc::{JsonRpcClient, JsonRpcClientImpl};
use crate::lotus::client::LotusJsonRPCClient;
use crate::manager::LotusSubnetManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard};

/// The subnet manager connection that holds the subnet config and the manager instance.
pub(crate) struct Connection<T: JsonRpcClient> {
    subnet: Subnet,
    manager: LotusSubnetManager<T>,
}

impl<T: JsonRpcClient> Connection<T> {
    pub fn subnet(&self) -> &Subnet {
        &self.subnet
    }

    pub fn manager(&self) -> &LotusSubnetManager<T> {
        &self.manager
    }
}

/// The json rpc subnet manager connection pool. This struct can be shared by all the subnet methods.
/// As such, there is no need to re-init the same SubnetManager for different methods to reuse connections.
pub(crate) struct SubnetManagerPool<T: JsonRpcClient> {
    config: Arc<ReloadableConfig>,
    connections: RwLock<HashMap<String, Connection<T>>>,
}

impl SubnetManagerPool<JsonRpcClientImpl> {
    pub fn from_reload_config(reload_config: Arc<ReloadableConfig>) -> Self {
        let config = reload_config.get_config();

        let mut connections = HashMap::new();
        for (_, subnet) in config.subnets.iter() {
            let manager = from_subnet(subnet);
            let id = subnet.id.to_string();
            let conn = Connection {
                manager,
                subnet: subnet.clone(),
            };

            connections.insert(id, conn);
        }

        Self {
            config: reload_config,
            connections: RwLock::new(connections),
        }
    }

    /// Get the connection instance for the subnet. If the `subnet_str` is not found in the
    /// pool, it will check in the latest config. If found in the config, it will create the new
    /// connection and insert to the pool. If it's still not found in the config, it returns None.
    pub async fn get(
        &self,
        subnet_str: &str,
    ) -> Option<RwLockReadGuard<Connection<JsonRpcClientImpl>>> {
        let connections = self.connections.read().await;

        let connections = if !connections.contains_key(subnet_str) {
            // we check if the latest config has that subnet
            let config = self.config.get_config();
            let subnet = match config.subnets.get(subnet_str) {
                // it's not found, return immediately
                None => return None,
                Some(subnet) => subnet,
            };

            // The new subnet is found in the config. We need to load the new subnet
            let manager = from_subnet(subnet);
            let connection = Connection {
                manager,
                subnet: subnet.clone(),
            };

            // we need a write lock to update the new connection
            self.connections
                .write()
                .await
                .insert(String::from(subnet_str), connection);

            // obtain the read lock to align with the return type
            self.connections.read().await
        } else {
            connections
        };

        let conn = RwLockReadGuard::map(connections, |connections| {
            connections.get(subnet_str).unwrap()
        });

        Some(conn)
    }
}

/// TO BE REPLACED AS A LIB CALL ONCE OTHER PR MERGED
fn lotus_from_subnet(subnet: &Subnet) -> LotusJsonRPCClient<JsonRpcClientImpl> {
    let url = subnet.jsonrpc_api_http.clone();
    let auth_token = subnet.auth_token.as_deref();
    let jsonrpc_client = JsonRpcClientImpl::new(url, auth_token);
    LotusJsonRPCClient::new(jsonrpc_client)
}

fn from_subnet(subnet: &Subnet) -> LotusSubnetManager<JsonRpcClientImpl> {
    let lotus = lotus_from_subnet(subnet);
    LotusSubnetManager::new(lotus)
}
