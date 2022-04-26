//! Storage related to a network supported by SubVT.
//! Each supported network has a separate database.
use crate::postgres::network::PostgreSQLNetworkStorage;
use std::collections::{HashMap, HashSet};
use subvt_types::{
    crypto::AccountId,
    rdb::ValidatorInfo,
    substrate::{Balance, Era, EraStakers, ValidatorPreferences, ValidatorStake},
};

type PostgresValidatorInfo = (
    Option<i64>,
    Option<i64>,
    i64,
    i64,
    i64,
    i64,
    i64,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<bool>,
    Option<i32>,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<bool>,
);

impl PostgreSQLNetworkStorage {
    pub async fn save_era(
        &self,
        era: &Era,
        total_stake: u128,
        era_stakers: &EraStakers,
    ) -> anyhow::Result<Option<i64>> {
        let nominator_count = {
            let mut nominator_account_id_set: HashSet<AccountId> = HashSet::new();
            for validator_stake in &era_stakers.stakers {
                for nominator_stake in &validator_stake.nominators {
                    nominator_account_id_set.insert(nominator_stake.account.id);
                }
            }
            nominator_account_id_set.len() as i64
        };
        let maybe_result: Option<(i64,)> = sqlx::query_as(
            r#"
            INSERT INTO sub_era (index, start_timestamp, end_timestamp, active_nominator_count, total_stake, minimum_stake, maximum_stake, average_stake, median_stake)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (index) DO NOTHING
            RETURNING index
            "#,
        )
            .bind(era.index)
            .bind(era.start_timestamp as i64)
            .bind(era.end_timestamp as i64)
            .bind(nominator_count)
            .bind(total_stake.to_string())
            .bind(era_stakers.min_stake().1.to_string())
            .bind(era_stakers.max_stake().1.to_string())
            .bind(era_stakers.average_stake().to_string())
            .bind(era_stakers.median_stake().to_string())
            .fetch_optional(&self.connection_pool)
            .await?;
        if let Some(result) = maybe_result {
            Ok(Some(result.0))
        } else {
            Ok(None)
        }
    }

    pub async fn save_epoch(&self, index: u64, era_index: u32) -> anyhow::Result<Option<i64>> {
        let maybe_result: Option<(i64,)> = sqlx::query_as(
            r#"
            INSERT INTO sub_epoch (index, era_index)
            VALUES ($1, $2)
            ON CONFLICT (index) DO NOTHING
            RETURNING index
            "#,
        )
        .bind(index as i64)
        .bind(era_index)
        .fetch_optional(&self.connection_pool)
        .await?;
        if let Some(result) = maybe_result {
            Ok(Some(result.0))
        } else {
            Ok(None)
        }
    }

    pub async fn save_era_validators(
        &self,
        era_index: u32,
        active_validator_account_ids: &[AccountId],
        all_validator_account_ids: &[AccountId],
        bonded_account_id_map: &HashMap<AccountId, AccountId>,
        validator_stake_map: &HashMap<AccountId, ValidatorStake>,
        validator_prefs_map: &HashMap<AccountId, ValidatorPreferences>,
    ) -> anyhow::Result<()> {
        let mut transaction = self.connection_pool.begin().await?;
        for validator_account_id in all_validator_account_ids {
            // create validator account (if not exists)
            sqlx::query(
                r#"
                INSERT INTO sub_account (id)
                VALUES ($1)
                ON CONFLICT (id) DO NOTHING
                "#,
            )
            .bind(validator_account_id.to_string())
            .execute(&mut transaction)
            .await?;
            // create controller account id (if not exists)
            let maybe_controller_account_id = bonded_account_id_map.get(validator_account_id);
            if let Some(controller_account_id) = maybe_controller_account_id {
                sqlx::query(
                    r#"
                    INSERT INTO sub_account (id)
                    VALUES ($1)
                    ON CONFLICT (id) DO NOTHING
                    "#,
                )
                .bind(controller_account_id.to_string())
                .execute(&mut transaction)
                .await?;
            }
            let maybe_active_validator_index = active_validator_account_ids
                .iter()
                .position(|account_id| account_id == validator_account_id);
            // get prefs
            let maybe_validator_prefs = validator_prefs_map.get(validator_account_id);
            // get stakes for active
            let maybe_validator_stake = validator_stake_map.get(validator_account_id);

            // create record (if not exists)
            sqlx::query(
                r#"
                INSERT INTO sub_era_validator (era_index, validator_account_id, controller_account_id, is_active, active_validator_index, commission_per_billion, blocks_nominations, self_stake, total_stake, active_nominator_count)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                ON CONFLICT (era_index, validator_account_id) DO NOTHING
                "#,
            )
                .bind(era_index)
                .bind(validator_account_id.to_string())
                .bind(maybe_controller_account_id.map(|id| id.to_string()))
                .bind(maybe_active_validator_index.is_some())
                .bind(maybe_active_validator_index.map(|index| index as i64))
                .bind(maybe_validator_prefs.map(|validator_prefs| validator_prefs.commission_per_billion))
                .bind(maybe_validator_prefs.map(|validator_prefs| validator_prefs.blocks_nominations))
                .bind(maybe_validator_stake.map(|validator_stake| validator_stake.self_stake.to_string()))
                .bind(maybe_validator_stake.map(|validator_stake| validator_stake.total_stake.to_string()))
                .bind(maybe_validator_stake.map(|validator_stake| validator_stake.nominators.len() as i32))
                .execute(&mut transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn save_era_stakers(&self, era_stakers: &EraStakers) -> anyhow::Result<()> {
        let mut transaction = self.connection_pool.begin().await?;
        for validator_stake in &era_stakers.stakers {
            sqlx::query(
                r#"
                INSERT INTO sub_account (id)
                VALUES ($1)
                ON CONFLICT (id) DO NOTHING
                "#,
            )
            .bind(validator_stake.account.id.to_string())
            .execute(&mut transaction)
            .await?;
            for nominator_stake in &validator_stake.nominators {
                // create nominator account (if not exists)
                sqlx::query(
                    r#"
                    INSERT INTO sub_account (id)
                    VALUES ($1)
                    ON CONFLICT (id) DO NOTHING
                    "#,
                )
                .bind(nominator_stake.account.id.to_string())
                .execute(&mut transaction)
                .await?;
                sqlx::query(
                    r#"
                    INSERT INTO sub_era_staker (era_index, validator_account_id, nominator_account_id, stake)
                    VALUES ($1, $2, $3, $4)
                    ON CONFLICT (era_index, validator_account_id, nominator_account_id) DO NOTHING
                    "#,
                )
                    .bind(era_stakers.era.index)
                    .bind(validator_stake.account.id.to_string())
                    .bind(nominator_stake.account.id.to_string())
                    .bind(nominator_stake.stake.to_string())
                    .execute(&mut transaction)
                    .await?;
            }
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn save_session_para_validators(
        &self,
        era_index: u32,
        session_index: u64,
        validator_account_ids: &[&AccountId],
    ) -> anyhow::Result<()> {
        let mut transaction = self.connection_pool.begin().await?;
        for validator_account_id in validator_account_ids {
            sqlx::query(
                r#"
                INSERT INTO sub_account (id)
                VALUES ($1)
                ON CONFLICT (id) DO NOTHING
                "#,
            )
            .bind(validator_account_id.to_string())
            .execute(&mut transaction)
            .await?;
            sqlx::query(
                r#"
                    INSERT INTO sub_session_para_validator (era_index, session_index, validator_account_id)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (session_index, validator_account_id) DO NOTHING
                    "#,
            )
                .bind(era_index)
                .bind(session_index as i64)
                .bind(validator_account_id.to_string())
                .execute(&mut transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn era_exists(&self, era_index: u32) -> anyhow::Result<bool> {
        let record_count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(index) FROM sub_era
            WHERE index = $1
            "#,
        )
        .bind(era_index)
        .fetch_one(&self.connection_pool)
        .await?;
        Ok(record_count.0 > 0)
    }

    pub async fn update_era_reward_points(
        &self,
        era_index: u32,
        total_reward_points: u32,
    ) -> anyhow::Result<Option<i64>> {
        let maybe_result: Option<(i64,)> = sqlx::query_as(
            r#"
            UPDATE sub_era SET total_reward_points = $1, updated_at = now()
            WHERE index = $2
            RETURNING index
            "#,
        )
        .bind(total_reward_points)
        .bind(era_index)
        .fetch_optional(&self.connection_pool)
        .await?;
        if let Some(result) = maybe_result {
            Ok(Some(result.0))
        } else {
            Ok(None)
        }
    }

    pub async fn update_era_total_validator_reward(
        &self,
        era_index: u32,
        total_validator_reward: Balance,
    ) -> anyhow::Result<Option<i64>> {
        let maybe_result: Option<(i64,)> = sqlx::query_as(
            r#"
            UPDATE sub_era SET total_validator_reward = $1, updated_at = now()
            WHERE index = $2
            RETURNING index
            "#,
        )
        .bind(total_validator_reward.to_string())
        .bind(era_index)
        .fetch_optional(&self.connection_pool)
        .await?;
        if let Some(result) = maybe_result {
            Ok(Some(result.0))
        } else {
            Ok(None)
        }
    }

    pub async fn update_era_validator_reward_points(
        &self,
        era_index: u32,
        era_reward_points_map: HashMap<AccountId, u32>,
    ) -> anyhow::Result<()> {
        let mut transaction = self.connection_pool.begin().await?;
        for (validator_account_id, reward_points) in era_reward_points_map {
            sqlx::query(
                r#"
                UPDATE sub_era_validator SET reward_points = $1, updated_at = now()
                WHERE era_index = $2 AND validator_account_id = $3
                "#,
            )
            .bind(reward_points)
            .bind(era_index)
            .bind(validator_account_id.to_string())
            .execute(&mut transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn get_validator_info(
        &self,
        block_hash: &str,
        validator_account_id: &AccountId,
        is_active: bool,
        era_index: u32,
    ) -> anyhow::Result<ValidatorInfo> {
        let validator_info: PostgresValidatorInfo = sqlx::query_as(
            r#"
            SELECT discovered_at, killed_at, slash_count, offline_offence_count, active_era_count, inactive_era_count, total_reward_points, unclaimed_eras, blocks_authored, reward_points, heartbeat_received, onekv_candidate_record_id, onekv_binary_version, onekv_rank, onekv_location, onekv_is_valid
            FROM sub_get_validator_info($1, $2, $3, $4)
            "#
        )
            .bind(block_hash)
            .bind(validator_account_id.to_string())
            .bind(is_active)
            .bind(era_index as i64)
            .fetch_one(&self.connection_pool)
            .await?;
        let mut unclaimed_era_indices: Vec<u32> = Vec::new();
        if let Some(concated_string) = validator_info.7 {
            for unclaimed_era_index_string in concated_string.split(',') {
                if let Ok(unclaimed_era_index) = unclaimed_era_index_string.parse::<u32>() {
                    unclaimed_era_indices.push(unclaimed_era_index);
                }
            }
        }
        unclaimed_era_indices.sort_unstable();
        Ok(ValidatorInfo {
            discovered_at: validator_info.0.map(|value| value as u64),
            killed_at: validator_info.1.map(|value| value as u64),
            slash_count: validator_info.2 as u64,
            offline_offence_count: validator_info.3 as u64,
            active_era_count: validator_info.4 as u64,
            inactive_era_count: validator_info.5 as u64,
            total_reward_points: validator_info.6 as u64,
            unclaimed_era_indices,
            blocks_authored: validator_info.8.map(|value| value as u64),
            reward_points: validator_info.9.map(|value| value as u64),
            heartbeat_received: validator_info.10,
            onekv_candidate_record_id: validator_info.11.map(|value| value as u32),
            onekv_binary_version: validator_info.12,
            onekv_rank: validator_info.13.map(|value| value as u64),
            onekv_location: validator_info.14,
            onekv_is_valid: validator_info.15,
        })
    }
}
