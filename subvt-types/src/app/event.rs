use crate::crypto::AccountId;

pub struct ValidatorOfflineEvent {
    pub id: u32,
    pub block_hash: String,
    pub event_index: Option<u32>,
    pub validator_account_id: AccountId,
}

pub struct ChilledEvent {
    pub id: u32,
    pub block_hash: String,
    pub extrinsic_index: Option<u32>,
    pub event_index: u32,
    pub stash_account_id: AccountId,
}