/// Substrate client structure and its functions.
/// This is the main gateway to a Substrate node through its RPC interface.

use crate::{
    metadata::Metadata,
    storage::{
        get_rpc_storage_map_params,
        get_rpc_storage_plain_params,
        get_rpc_paged_keys_params,
        get_storage_map_key,
        get_rpc_paged_map_keys_params,
    },
};
use jsonrpsee::{ws_client::{WsClient, WsClientBuilder}};
use jsonrpsee_types::{
    Subscription,
    traits::{Client, SubscriptionClient},
    v2::params::JsonRpcParams,
};
use log::{debug, error};
use parity_scale_codec::Decode;
use std::str::FromStr;
use subvt_types::crypto::AccountId;
use subvt_types::substrate::{
    Account, BlockHeader, Chain, Era, Epoch, EraRewardPoints, EraStakers, IdentityRegistration,
    Nomination, RewardDestination, Stake, SystemProperties, ValidatorPreferences, ValidatorStake,
};
use subvt_utility::decode_hex_string;
use sp_core::storage::{StorageChangeSet, StorageKey};
use std::collections::HashMap;
use std::convert::TryInto;
use subvt_types::subvt::InactiveValidator;
use subvt_types::substrate::SuperAccountId;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

mod storage;
mod metadata;

/// The client.
pub struct SubstrateClient {
    pub chain: Chain,
    pub metadata: Metadata,
    pub system_properties: SystemProperties,
    ws_client: WsClient,
}

impl SubstrateClient {
    /// Connect to the node and construct a new Substrate client.
    pub async fn new(
        rpc_ws_url: String,
        connection_timeout_seconds: u64,
        request_timeout_seconds: u64,
    ) -> anyhow::Result<Self> {
        debug!("Constructing the Substrate client.");
        let ws_client = WsClientBuilder::default()
            .connection_timeout(std::time::Duration::from_secs(connection_timeout_seconds))
            .request_timeout(std::time::Duration::from_secs(request_timeout_seconds))
            .build(rpc_ws_url.as_str()).await?;
        debug!("Substrate connection successful.");
        let metadata = {
            let metadata_response: String = ws_client.request(
                "state_getMetadata",
                JsonRpcParams::NoParams,
            ).await?;
            Metadata::from(metadata_response.as_str())?
        };
        debug!("Got metadata. {:?}", metadata.runtime_config);
        let system_properties: SystemProperties = ws_client.request(
            "system_properties",
            JsonRpcParams::NoParams,
        ).await?;
        debug!("Got system properties. {:?}", system_properties);
        let chain: String = ws_client.request(
            "system_chain",
            JsonRpcParams::NoParams,
        ).await?;
        let chain = Chain::from_str(chain.as_str())?;
        Ok(
            Self {
                chain,
                metadata,
                system_properties,
                ws_client,
            }
        )
    }

    pub async fn get_current_block_hash(&self) -> anyhow::Result<String> {
        let hash = self.ws_client.request(
            "chain_getBlockHash",
            JsonRpcParams::NoParams,
        ).await?;
        Ok(hash)
    }

    /// Get a block hash by its number.
    pub async fn get_block_hash(&self, block_number: u64) -> anyhow::Result<String> {
        let hash = self.ws_client.request(
            "chain_getBlockHash",
            JsonRpcParams::Array(vec![block_number.into()]),
        ).await?;
        Ok(hash)
    }

    /// Get a block header by its hash.
    pub async fn get_block_header(&self, block_hash: &str) -> anyhow::Result<BlockHeader> {
        let header = self.ws_client.request(
            "chain_getHeader",
            JsonRpcParams::Array(vec![block_hash.into()]),
        ).await?;
        Ok(header)
    }

    /// Get the hash of the current finalized block.
    pub async fn get_finalized_block_hash(&self) -> anyhow::Result<String> {
        let hash: String = self.ws_client.request(
            "chain_getFinalizedHead",
            JsonRpcParams::NoParams,
        ).await?;
        Ok(hash)
    }

    /// Get active era at the given block.
    pub async fn get_active_era(&self, block_hash: &str) -> anyhow::Result<Era> {
        let hex_string: String = self.ws_client.request(
            "state_getStorage",
            get_rpc_storage_plain_params(
                "Staking",
                "ActiveEra",
                Some(block_hash),
            ),
        ).await?;
        let active_era_info = Era::from(
            hex_string.as_str(),
            self.metadata.runtime_config.era_duration_millis,
        )?;
        Ok(active_era_info)
    }

    /// Get current epoch at the given block.
    pub async fn get_current_epoch(&self, block_hash: &str) -> anyhow::Result<Epoch> {
        let index: u64 = {
            let hex_string: String = self.ws_client.request(
                "state_getStorage",
                get_rpc_storage_plain_params(
                    "Babe",
                    "EpochIndex",
                    Some(block_hash),
                ),
            ).await?;
            decode_hex_string(hex_string.as_str())?
        };
        let start_block_number = {
            let hex_string: String = self.ws_client.request(
                "state_getStorage",
                get_rpc_storage_plain_params(
                    "Babe",
                    "EpochStart",
                    Some(block_hash),
                ),
            ).await?;
            decode_hex_string::<(u32, u32)>(hex_string.as_str())?.1
        };
        let start_block_hash = self.get_block_hash(start_block_number as u64).await?;
        let start_timestamp_millis: u64 = {
            let hex_string: String = self.ws_client.request(
                "state_getStorage",
                get_rpc_storage_plain_params(
                    "Timestamp",
                    "Now",
                    Some(start_block_hash.as_str()),
                ),
            ).await?;
            decode_hex_string(hex_string.as_str())?
        };
        let start_timestamp = start_timestamp_millis / 1000;
        let end_timestamp_millis = start_timestamp_millis + self.metadata.runtime_config.epoch_duration_millis;
        let end_timestamp = end_timestamp_millis / 1000;
        Ok(
            Epoch {
                index,
                start_block_number,
                start_timestamp,
                end_timestamp,
            }
        )
    }

    fn account_id_from_storage_key_string(&self, storage_key_string: &str) -> AccountId {
        let hex_string = &storage_key_string[(storage_key_string.len() - 64)..];
        decode_hex_string(hex_string).unwrap()
    }

    fn account_id_from_storage_key(&self, storage_key: &StorageKey) -> AccountId {
        storage_key.0[storage_key.0.len() - 32..].try_into().unwrap()
    }

    /// Get the list of all active validators at the given block.
    pub async fn get_active_validator_account_ids(
        &self,
        block_hash: &str,
    ) -> anyhow::Result<Vec<AccountId>> {
        let hex_string: String = self.ws_client.request(
            "state_getStorage",
            get_rpc_storage_plain_params(
                "Session",
                "Validators",
                Some(block_hash),
            ),
        ).await?;
        Ok(decode_hex_string(hex_string.as_str())?)
    }

    /// Get the list of all validators at the given block.
    pub async fn get_all_inactive_validators(
        &self,
        block_hash: &str,
    ) -> anyhow::Result<Vec<InactiveValidator>> {
        debug!("start");
        let active_validator_account_ids = self.get_active_validator_account_ids(block_hash).await?;
        let keys_page_size = 1000;
        let max_nominator_rewarded_per_validator: u32 = self.metadata.module("Staking")?
            .constant("MaxNominatorRewardedPerValidator")?
            .value()?;
        let all_keys: Vec<String> = {
            let mut all_keys: Vec<String> = Vec::new();
            loop {
                let last = all_keys.last();
                let mut keys: Vec<String> = self.ws_client.request(
                    "state_getKeysPaged",
                    get_rpc_paged_keys_params(
                        "Staking",
                        "Validators",
                        keys_page_size,
                        if let Some(last) = last { Some(last.as_str()) } else { None },
                        Some(block_hash),
                    ),
                ).await?;
                let keys_length = keys.len();
                all_keys.append(&mut keys);
                if keys_length < keys_page_size {
                    break;
                }
            }
            all_keys.iter()
                .map(|key| key.to_owned())
                .filter(|key| {
                    !active_validator_account_ids.contains(&self.account_id_from_storage_key_string(key))
                })
                .collect()
        };

        let mut inactive_validator_map: HashMap<AccountId, InactiveValidator> = HashMap::new();
        for key in &all_keys {
            let account_id = self.account_id_from_storage_key_string(key);
            let inactive_validator = InactiveValidator {
                account: Account { id: account_id.clone(), ..Default::default() },
                active_next_session: false,
                ..Default::default()
            };
            inactive_validator_map.insert(account_id, inactive_validator);
        }
        debug!("There are {} inactive validators.", inactive_validator_map.len());
        // get next session keys
        {
            debug!("Get next session keys for all validators.");
            let keys: Vec<String> = inactive_validator_map.values().map(
                |validator| {
                    get_storage_map_key(
                        &self.metadata,
                        "Session",
                        "NextKeys",
                        &validator.account.id,
                    )
                }
            ).collect();
            for chunk in keys.chunks(keys_page_size) {
                let chunk_values: Vec<StorageChangeSet<String>> = self.ws_client.request(
                    "state_queryStorageAt",
                    JsonRpcParams::Array(
                        vec![
                            chunk.into(),
                            block_hash.into(),
                        ]
                    ),
                ).await?;

                for (storage_key, data) in chunk_values[0].changes.iter() {
                    if let Some(data) = data {
                        let account_id = self.account_id_from_storage_key(storage_key);
                        let session_keys = format!("0x{}", hex::encode(&data.0));
                        let validator = inactive_validator_map.get_mut(&account_id).unwrap();
                        validator.next_session_keys = session_keys;
                    }
                }
            }
            debug!("Got next session keys.");
        }
        // get next session active validator keys
        {
            debug!("Get queued keys for the next session.");
            let hex_string: String = self.ws_client.request(
                "state_getStorage",
                get_rpc_storage_plain_params(
                    "Session",
                    "QueuedKeys",
                    Some(block_hash),
                ),
            ).await?;
            let session_key_pairs: Vec<(AccountId, [u8; 192])> = decode_hex_string(&hex_string).unwrap();
            for session_key_pair in session_key_pairs.iter() {
                let session_keys = format!("0x{}", hex::encode(session_key_pair.1));
                if let Some(validator) = inactive_validator_map.get_mut(&session_key_pair.0) {
                    validator.active_next_session = validator.next_session_keys == session_keys;
                }
            }
            debug!("Got {} queued session keys for the next session.", session_key_pairs.len());
        }
        // get reward destinations
        {
            debug!("Get reward destinations.");
            let keys: Vec<String> = inactive_validator_map.values().map(
                |validator| {
                    get_storage_map_key(
                        &self.metadata,
                        "Staking",
                        "Payee",
                        &validator.account.id,
                    )
                }
            ).collect();

            for chunk in keys.chunks(keys_page_size) {
                let chunk_values: Vec<StorageChangeSet<String>> = self.ws_client.request(
                    "state_queryStorageAt",
                    JsonRpcParams::Array(
                        vec![
                            chunk.into(),
                            block_hash.into(),
                        ]
                    ),
                ).await?;

                for (storage_key, data) in chunk_values[0].changes.iter() {
                    if let Some(data) = data {
                        let account_id = self.account_id_from_storage_key(storage_key);
                        let bytes: &[u8] = &data.0;
                        let reward_destination = RewardDestination::from_bytes(bytes).unwrap();
                        let validator = inactive_validator_map.get_mut(&account_id).unwrap();
                        validator.reward_destination = reward_destination;
                    }
                }
            }
            debug!("Got reward destinations.");
        }
        // get slashings
        {
            debug!("Get slashings.");
            let keys: Vec<String> = inactive_validator_map.values().map(
                |validator| {
                    get_storage_map_key(
                        &self.metadata,
                        "Staking",
                        "ValidatorSlashInEra",
                        &validator.account.id,
                    )
                }
            ).collect();
            let mut number_of_slashed_validators = 0;
            for chunk in keys.chunks(keys_page_size) {
                let chunk_values: Vec<StorageChangeSet<String>> = self.ws_client.request(
                    "state_queryStorageAt",
                    JsonRpcParams::Array(
                        vec![
                            chunk.into(),
                            block_hash.into(),
                        ]
                    ),
                ).await?;

                for (storage_key, data) in chunk_values[0].changes.iter() {
                    if data.is_some() {
                        let account_id = self.account_id_from_storage_key(storage_key);
                        if let Some(validator) = inactive_validator_map.get_mut(&account_id) {
                            number_of_slashed_validators += 1;
                            validator.slashed = true;
                        }
                    }
                }
            }
            debug!("Got {} slashings.", number_of_slashed_validators);
        }

        // get nominations
        {
            let mut all_keys: Vec<String> = Vec::new();
            loop {
                let last = all_keys.last();
                let mut keys: Vec<String> = self.ws_client.request(
                    "state_getKeysPaged",
                    get_rpc_paged_keys_params(
                        "Staking",
                        "Nominators",
                        keys_page_size,
                        if let Some(last) = last { Some(last.as_str()) } else { None },
                        Some(block_hash),
                    ),
                ).await?;
                let keys_length = keys.len();
                all_keys.append(&mut keys);
                if keys_length < keys_page_size {
                    break;
                }
            }

            debug!("{} nominations.", all_keys.len());
            let mut nomination_map: HashMap<AccountId, Nomination> = HashMap::new();
            for chunk in all_keys.chunks(keys_page_size) {
                let chunk_values: Vec<StorageChangeSet<String>> = self.ws_client.request(
                    "state_queryStorageAt",
                    JsonRpcParams::Array(
                        vec![
                            chunk.into(),
                            block_hash.into(),
                        ]
                    ),
                ).await?;
                for (storage_key, data) in chunk_values[0].changes.iter() {
                    if let Some(data) = data {
                        let account_id = self.account_id_from_storage_key(storage_key);
                        let bytes: &[u8] = &data.0;
                        let nomination = Nomination::from_bytes(
                            bytes,
                            account_id,
                        ).unwrap();
                        nomination_map.insert(
                            nomination.nominator_account.id.clone(),
                            nomination,
                        );
                    }
                }
            }
            debug!("Got {} nominations.", nomination_map.len());

            let mut controller_storage_keys: Vec<String> = nomination_map.keys().map(
                |nominator_account_id|
                    get_storage_map_key(
                        &self.metadata,
                        "Staking",
                        "Bonded",
                        &nominator_account_id,
                    )
            ).collect();
            // add validator addresses
            for validator_address in inactive_validator_map.keys() {
                controller_storage_keys.push(
                    get_storage_map_key(
                        &self.metadata,
                        "Staking",
                        "Bonded",
                        validator_address,
                    )
                )
            }
            let mut controller_account_ids: Vec<AccountId> = Vec::new();
            for chunk in controller_storage_keys.chunks(keys_page_size) {
                let chunk_values: Vec<StorageChangeSet<String>> = self.ws_client.request(
                    "state_queryStorageAt",
                    JsonRpcParams::Array(
                        vec![
                            chunk.into(),
                            block_hash.into(),
                        ]
                    ),
                ).await?;
                for (storage_key, data) in chunk_values[0].changes.iter() {
                    if let Some(data) = data {
                        let mut bytes: &[u8] = &data.0;
                        let controller_account_id: AccountId = Decode::decode(&mut bytes).unwrap();
                        let account_id = self.account_id_from_storage_key(storage_key);
                        controller_account_ids.push(controller_account_id.clone());
                        let controller_account = Account { id: controller_account_id, ..Default::default() };
                        if let Some(nomination) = nomination_map.get_mut(&account_id) {
                            nomination.controller_account = controller_account;
                        } else {
                            let validator = inactive_validator_map.get_mut(&account_id).unwrap();
                            validator.controller_account = controller_account;
                        }
                    }
                }
            }
            debug!("Got {} controller account ids.", controller_account_ids.len());

            // her biri için bonding'i al (staking.bonded)
            let ledger_storage_keys: Vec<String> = controller_account_ids.iter().map(
                |controller_account_id|
                    get_storage_map_key(
                        &self.metadata,
                        "Staking",
                        "Ledger",
                        &controller_account_id,
                    )
            ).collect();
            // her biri için bonded miktarı al (staking.ledger)
            for chunk in ledger_storage_keys.chunks(keys_page_size) {
                let chunk_values: Vec<StorageChangeSet<String>> = self.ws_client.request(
                    "state_queryStorageAt",
                    JsonRpcParams::Array(
                        vec![
                            chunk.into(),
                            block_hash.into(),
                        ]
                    ),
                ).await?;
                for (_, data) in chunk_values[0].changes.iter() {
                    if let Some(data) = data {
                        let bytes: &[u8] = &data.0;
                        let stake: Stake = Stake::from_bytes(bytes).unwrap();
                        let account_id = &stake.stash_account.id;
                        if let Some(nomination) = nomination_map.get_mut(account_id) {
                            nomination.stake = stake;
                        } else {
                            let validator = inactive_validator_map.get_mut(account_id).unwrap();
                            validator.self_stake = stake;
                        }
                    }
                }
            }
            debug!("Got all stakes.");
            for nomination in nomination_map.values() {
                for account_id in nomination.target_account_ids.iter() {
                    if let Some(validator) = inactive_validator_map.get_mut(account_id) {
                        validator.nominations.push(nomination.clone());
                        validator.oversubscribed =
                            validator.nominations.len()
                                > max_nominator_rewarded_per_validator as usize;
                    }
                }
            }
            for validator in inactive_validator_map.values_mut() {
                validator.nominations.sort_by_key(|nomination| {
                    let mut hasher = DefaultHasher::new();
                    nomination.nominator_account.id.hash(&mut hasher);
                    hasher.finish()
                });
            }
        }
        // get identities
        {
            let keys: Vec<String> = inactive_validator_map.values().into_iter().map(
                |inactive_validator| {
                    get_storage_map_key(
                        &self.metadata,
                        "Identity",
                        "IdentityOf",
                        &inactive_validator.account.id,
                    )
                }
            ).collect();
            debug!("Got {} keys for id.", keys.len());
            let values: Vec<StorageChangeSet<String>> = self.ws_client.request(
                "state_queryStorageAt",
                JsonRpcParams::Array(
                    vec![
                        keys.into(),
                        block_hash.into(),
                    ]
                ),
            ).await?;
            debug!("Got {} identities. Transform.", values[0].changes.len());
            for (storage_key, storage_data) in values[0].changes.iter() {
                let account_id = self.account_id_from_storage_key(storage_key);
                let validator = inactive_validator_map.get_mut(&account_id).unwrap();
                let identity = match storage_data {
                    Some(data) => {
                        let bytes: &[u8] = &data.0;
                        Some(IdentityRegistration::from_bytes(bytes).unwrap())
                    }
                    None => None
                };
                validator.account.identity = identity;
            }
        }
        // get super identities
        {
            let keys: Vec<String> = inactive_validator_map.values().into_iter().map(
                |inactive_validator| {
                    get_storage_map_key(
                        &self.metadata,
                        "Identity",
                        "SuperOf",
                        &inactive_validator.account.id,
                    )
                }
            ).collect();
            debug!("Got {} keys for super id.", keys.len());
            let values: Vec<StorageChangeSet<String>> = self.ws_client.request(
                "state_queryStorageAt",
                JsonRpcParams::Array(
                    vec![
                        keys.into(),
                        block_hash.into(),
                    ]
                ),
            ).await?;
            debug!("Got {} optional super ids.", values[0].changes.len());
            let mut super_account_id_map: HashMap<AccountId, AccountId> = HashMap::new();
            for (storage_key, storage_data) in values[0].changes.iter() {
                if let Some(data) = storage_data {
                    let account_id = self.account_id_from_storage_key(storage_key);
                    let mut bytes: &[u8] = &data.0;
                    let super_identity: SuperAccountId = Decode::decode(&mut bytes).unwrap();
                    super_account_id_map.insert(account_id, super_identity.0);
                }
            }
            debug!("Got {} super accounts. Get identities for super accounts.", super_account_id_map.len());
            let keys: Vec<String> = super_account_id_map.values().map(
                |super_account_entry| {
                    get_storage_map_key(
                        &self.metadata,
                        "Identity",
                        "IdentityOf",
                        super_account_entry,
                    )
                }
            ).collect();

            // let storage_keys: Vec<String> = keys.iter().map(|key| key.1.clone()).collect();
            let values: Vec<StorageChangeSet<String>> = self.ws_client.request(
                "state_queryStorageAt",
                JsonRpcParams::Array(
                    vec![
                        keys.into(),
                        block_hash.into(),
                    ]
                ),
            ).await?;
            debug!("Got {} super identities. Transform.", values[0].changes.len());

            for (storage_key, data) in &values[0].changes {
                if let Some(data) = data {
                    let super_account_id = self.account_id_from_storage_key(storage_key);
                    let bytes: &[u8] = &data.0;
                    let super_account_identity = IdentityRegistration::from_bytes(bytes).unwrap();
                    for pair in super_account_id_map.iter() {
                        if *pair.1 == super_account_id {
                            let validator = inactive_validator_map.get_mut(pair.0).unwrap();
                            let parent_identity = Some(super_account_identity.clone());
                            let parent_account = Account {
                                id: super_account_id.clone(),
                                identity: parent_identity,
                                parent: Box::new(None),
                            };
                            validator.account.parent = Box::new(Some(parent_account));
                        }
                    }
                }
            }
        }

        // get validator prefs
        {
            debug!("start :: get validator prefs all");
            let values: Vec<StorageChangeSet<String>> = self.ws_client.request(
                "state_queryStorageAt",
                JsonRpcParams::Array(
                    vec![
                        all_keys.into(),
                        block_hash.into(),
                    ]
                ),
            ).await?;
            for (storage_key, data) in values[0].changes.iter() {
                if let Some(data) = data {
                    let bytes: &[u8] = &data.0;
                    let preferences = ValidatorPreferences::from_bytes(bytes).unwrap();
                    let validator_account_id = self.account_id_from_storage_key(storage_key);
                    let validator = inactive_validator_map.get_mut(&validator_account_id).unwrap();
                    validator.preferences = preferences;
                }
            }
            debug!("Got all validator prefs.");
        }
        debug!("It's done baby!");
        Ok(
            inactive_validator_map.into_iter().map(
                |(_, validator)|
                    validator
            ).collect()
        )
    }

    /// Get the number of all validation intents at the given block.
    pub async fn get_total_validator_count(
        &self,
        block_hash: &str,
    ) -> anyhow::Result<u32> {
        let hex_string: String = self.ws_client.request(
            "state_getStorage",
            get_rpc_storage_plain_params(
                "Staking",
                "CounterForValidators",
                Some(block_hash),
            ),
        ).await?;
        decode_hex_string(hex_string.as_str())
    }

    /// Get total rewards earned by validators in the native currency at the given era.
    pub async fn get_era_total_validator_reward(
        &self,
        era_index: u32,
    ) -> anyhow::Result<u128> {
        let params = get_rpc_storage_map_params(
            &self.metadata,
            "Staking",
            "ErasValidatorReward",
            &era_index,
            None,
        );
        let hex_string: String = self.ws_client.request(
            "state_getStorage",
            params,
        ).await?;
        decode_hex_string(hex_string.as_str())
    }

    /// Get all the active stakes for the given era.
    pub async fn get_era_stakers(
        &self,
        era: &Era,
        clipped: bool,
        block_hash: &str,
    ) -> anyhow::Result<EraStakers> {
        let keys_page_size = 1000;
        let mut all_keys: Vec<String> = Vec::new();
        loop {
            let last = all_keys.last();
            let mut keys: Vec<String> = self.ws_client.request(
                "state_getKeysPaged",
                get_rpc_paged_map_keys_params(
                    &self.metadata,
                    "Staking",
                    if clipped { "ErasStakersClipped" } else { "ErasStakers" },
                    &era.index,
                    keys_page_size,
                    if let Some(last) = last { Some(last.as_str()) } else { None },
                    Some(block_hash),
                ),
            ).await?;
            let keys_length = keys.len();
            all_keys.append(&mut keys);
            if keys_length < keys_page_size {
                break;
            }
        }

        let mut stakers: Vec<ValidatorStake> = Vec::new();
        for chunk in all_keys.chunks(keys_page_size) {
            let chunk_values: Vec<StorageChangeSet<String>> = self.ws_client.request(
                "state_queryStorageAt",
                JsonRpcParams::Array(
                    vec![
                        chunk.into(),
                        block_hash.into(),
                    ]
                ),
            ).await?;

            for (storage_key, data) in chunk_values[0].changes.iter() {
                if let Some(data) = data {
                    let validator_account_id = self.account_id_from_storage_key(storage_key);
                    let nomination = ValidatorStake::from_bytes(
                        &data.0,
                        validator_account_id,
                    ).unwrap();
                    stakers.push(nomination);
                }
            }
        }
        stakers.sort_by_key(|validator_stake| validator_stake.total_stake);
        Ok(
            EraStakers {
                era: era.clone(),
                stakers,
            }
        )
    }

    /// Get total and individual era reward points earned by validators at the given era.
    /// Will give the points earned so far for an active era.
    pub async fn get_era_reward_points(
        &self,
        era_index: u32,
    ) -> anyhow::Result<EraRewardPoints> {
        let params = get_rpc_storage_map_params(
            &self.metadata,
            "Staking",
            "ErasRewardPoints",
            &era_index,
            None,
        );
        let hex_string: String = self.ws_client.request(
            "state_getStorage",
            params,
        ).await?;
        Ok(decode_hex_string(hex_string.as_str())?)
    }

    /// Get the session index at the given block.
    pub async fn get_current_session_index(
        &self,
        block_hash: &str,
    ) -> anyhow::Result<u32> {
        let hex_string: String = self.ws_client.request(
            "state_getStorage",
            get_rpc_storage_plain_params(
                "Session",
                "CurrentIndex",
                Some(block_hash),
            ),
        ).await?;
        decode_hex_string(hex_string.as_str())
    }

    async fn subscribe_to_blocks<F>(
        &self,
        subscribe_method_name: &str,
        unsubscribe_method_name: &str,
        callback: F,
    ) -> anyhow::Result<()>
        where F: Fn(BlockHeader)
    {
        let mut subscription: Subscription<BlockHeader> = self.ws_client.subscribe(
            subscribe_method_name,
            JsonRpcParams::NoParams,
            unsubscribe_method_name,
        ).await?;
        loop {
            let block_header = subscription.next().await?;
            match block_header {
                Some(header) => {
                    callback(header)
                }
                None => {
                    error!("Empty block header. Will exit new block subscription.");
                    break;
                }
            }
        }
        Ok(())
    }

    /// Subscribes to new blocks.
    pub async fn subscribe_to_new_blocks<F>(&self, callback: F) -> anyhow::Result<()>
        where F: Fn(BlockHeader)
    {
        self.subscribe_to_blocks(
            "chain_subscribeNewHeads",
            "chain_unsubscribeNewHeads",
            callback,
        ).await
    }

    /// Subscribes to finalized blocks.
    pub async fn subscribe_to_finalized_blocks<F>(&self, callback: F) -> anyhow::Result<()>
        where F: Fn(BlockHeader)
    {
        self.subscribe_to_blocks(
            "chain_subscribeFinalizedHeads",
            "chain_unsubscribeFinalizedHeads",
            callback,
        ).await
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
