use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::Result;
use cid::Cid;
use fil_actors_runtime::cbor;
use futures_util::future::join_all;
use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::MethodNum;
use ipc_gateway::Checkpoint;
use ipc_sdk::subnet_id::SubnetID;
use primitives::TCid;
use tokio::time::sleep;

use crate::config::Subnet;
use crate::lotus::client::LotusJsonRPCClient;
use crate::lotus::message::mpool::MpoolPushMessage;
use crate::lotus::LotusClient;

/// Starts the checkpoint manager.
pub async fn start(subnets: HashMap<String, Subnet>) {
    let managed_subnets = subnets_to_manage(subnets);

    let mut futures = Vec::new();
    for (child, parent) in managed_subnets {
        futures.push(manage_subnet(child, parent));
    }
    join_all(futures).await;
}

/// This function takes a `HashMap<String, Subnet>` and returns a `Vec` of tuples of the form
/// `(child_subnet, parent_subnet)`, where `child_subnet` is a subnet that we need to actively
/// manage checkpoint for. This means that for each `child_subnet` there exists at least one account
/// for which we need to submit checkpoints on behalf of to `parent_subnet`, which must also be
/// present in the map.
fn subnets_to_manage(subnets: HashMap<String, Subnet>) -> Vec<(Subnet, Subnet)> {
    // First, we remap subnets by SubnetID.
    let subnets_by_id: HashMap<SubnetID, Subnet> = subnets
        .values()
        .map(|s| (s.id.clone(), s.clone()))
        .collect();

    // Then, we filter for subnets that have at least one account and for which the parent subnet
    // is also in the map, and map into a Vec of (child_subnet, parent_subnet) tuples.
    subnets_by_id
        .values()
        .filter(|s| !s.accounts.is_empty())
        .filter(|s| s.id.parent().is_some() && subnets_by_id.contains_key(&s.id.parent().unwrap()))
        .map(|s| (s.clone(), subnets_by_id[&s.id.parent().unwrap()].clone()))
        .collect()
}

/// Monitors a subnet `child` for checkpoint blocks. It emits an event for every new checkpoint block.
async fn manage_subnet(child: Subnet, parent: Subnet) -> Result<()> {
    let child_client = LotusJsonRPCClient::from_subnet(&child);
    let parent_client = LotusJsonRPCClient::from_subnet(&parent);

    // Read the parent's chain head and obtain the tip set CID.
    let parent_head = parent_client.chain_head().await?;
    let cid_map = parent_head.cids.last().unwrap().clone();
    let parent_tip_set = Cid::try_from(cid_map)?;

    // Extract the checkpoint period from the state of the subnet actor in the parent.
    let state = parent_client
        .ipc_read_subnet_actor_state(parent_tip_set)
        .await?;
    let period = state.check_period;

    // We should have a way of knowing whether the validator has voted in the current open
    // checkpoint epoch. For now, we maintain this information in memory and upon a restart only
    // vote for the next epoch.

    // We can now start looping. In each loop we read the child subnet's chain head and check if
    // it's a checkpoint epoch. If it is, we construct and submit a checkpoint.
    loop {
        let child_head = child_client.chain_head().await?;
        let epoch: ChainEpoch = ChainEpoch::try_from(child_head.height)?;
        if epoch % period == 0 {
            // It's a checkpointing epoch and we may have checkpoints to submit.

            // First, we check which accounts are in the validator set. This is done by reading
            // the parent's chain head and requesting the state at that tip set.
            let parent_head = parent_client.chain_head().await?;
            // A key assumption we make now is that each block has exactly one tip set. We panic
            // if this is not the case as it violates our assumption.
            assert_eq!(parent_head.cids.len(), 1);
            let cid_map = parent_head.cids.first().unwrap().clone();
            let parent_tip_set = Cid::try_from(cid_map)?;

            let subnet_actor_state = parent_client
                .ipc_read_subnet_actor_state(parent_tip_set)
                .await?;
            let validator_set: HashSet<Address, RandomState> =
                HashSet::from_iter(subnet_actor_state.validator_set.iter().map(|v| v.addr));

            // Now, for each account defined in the `child` subnet that is in the validator set, we
            // submit a checkpoint on its behalf.
            assert_eq!(child_head.cids.len(), 1); // Again, check key assumption
            let child_tip_set = Cid::try_from(child_head.cids.first().unwrap().clone())?;
            for account in child.accounts.iter() {
                if validator_set.contains(&account) {
                    submit_checkpoint(child_tip_set, epoch, account, &child, &parent).await?;
                }
            }
        }

        // Sleep for an appropriate amount of time before checking the chain head again.
        sleep(Duration::from_secs(10)).await;
    }
}

/// Submits a checkpoint for `epoch` on behalf of `account` to the subnet actor of `child_subnet`
/// deployed on the parent subnet.
async fn submit_checkpoint(
    child_tip_set: Cid,
    epoch: ChainEpoch,
    account: &Address,
    child_subnet: &Subnet,
    parent_subnet: &Subnet,
) -> Result<()> {
    let mut checkpoint = Checkpoint::new(child_subnet.id.clone(), epoch);

    // Get the children checkpoints from the template on the gateway actor of the child subnet.
    let child_client = LotusJsonRPCClient::from_subnet(child_subnet);
    let template = child_client.ipc_get_checkpoint_template(epoch).await?;
    checkpoint.data.children = template.data.children;

    // Get the CID of previous checkpoint of the child subnet from the gateway actor of the parent
    // subnet.
    let parent_client = LotusJsonRPCClient::from_subnet(parent_subnet);
    let response = parent_client
        .ipc_get_prev_checkpoint_for_child(child_subnet.id.clone())
        .await?;
    let cid = Cid::try_from(response.cid)?;
    checkpoint.data.prev_check = TCid::from(cid);

    // The "tip_set".
    checkpoint.data.tip_set = child_tip_set.to_bytes();

    // The checkpoint is constructed. Now we call the `submit_checkpoint` method on the subnet actor
    // of the child subnet that is deployed on the parent subnet.
    let to = child_subnet.id.subnet_actor();
    let from = account.clone();
    let message = MpoolPushMessage::new(
        to,
        from,
        ipc_subnet_actor::Method::SubmitCheckpoint as MethodNum,
        cbor::serialize(&checkpoint, "checkpoint")?.to_vec(),
    );
    parent_client.mpool_push_message(message).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::manager::checkpoint::start;

    #[tokio::test]
    async fn test_checkpoint() {
        let config = Config::from_file("config/template.toml").unwrap();
        start(config.subnets).await;
    }
}
