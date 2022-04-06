//! Non-Substrate application events storage, such as new validator on network, 1KV rank change,
//! lost/new/changed nomination, etc.
use crate::postgres::network::PostgreSQLNetworkStorage;
use subvt_types::{app::app_event, crypto::AccountId};

impl PostgreSQLNetworkStorage {
    pub async fn save_new_validator_event(
        &self,
        validator_account_id: &AccountId,
        discovered_block_number: u64,
    ) -> anyhow::Result<u32> {
        self.save_account(validator_account_id).await?;
        let result: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO sub_app_event_new_validator (validator_account_id, discovered_block_number)
            VALUES ($1, $2)
            RETURNING id
            "#,
        )
        .bind(validator_account_id.to_string())
        .bind(discovered_block_number as i64)
        .fetch_one(&self.connection_pool)
        .await?;
        Ok(result.0 as u32)
    }

    pub async fn save_removed_validator_event(
        &self,
        validator_account_id: &AccountId,
        discovered_block_number: u64,
    ) -> anyhow::Result<u32> {
        self.save_account(validator_account_id).await?;
        let result: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO sub_app_event_removed_validator (validator_account_id, discovered_block_number)
            VALUES ($1, $2)
            RETURNING id
            "#,
        )
        .bind(validator_account_id.to_string())
        .bind(discovered_block_number as i64)
        .fetch_one(&self.connection_pool)
        .await?;
        Ok(result.0 as u32)
    }

    pub async fn save_new_nomination_event(
        &self,
        event: &app_event::NewNomination,
    ) -> anyhow::Result<u32> {
        self.save_account(&event.validator_account_id).await?;
        self.save_account(&event.nominator_stash_account_id).await?;
        let result: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO sub_app_event_new_nomination (validator_account_id, discovered_block_number, nominator_stash_account_id, active_amount, total_amount, nominee_count, is_onekv)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
            "#,
        )
            .bind(event.validator_account_id.to_string())
            .bind(event.discovered_block_number as i64)
            .bind(event.nominator_stash_account_id.to_string())
            .bind(event.active_amount.to_string())
            .bind(event.total_amount.to_string())
            .bind(event.nominee_count as i64)
            .bind(event.is_onekv)
            .fetch_one(&self.connection_pool)
            .await?;
        Ok(result.0 as u32)
    }

    pub async fn save_lost_nomination_event(
        &self,
        event: &app_event::LostNomination,
    ) -> anyhow::Result<u32> {
        self.save_account(&event.validator_account_id).await?;
        self.save_account(&event.nominator_stash_account_id).await?;
        let result: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO sub_app_event_lost_nomination (validator_account_id, discovered_block_number, nominator_stash_account_id, active_amount, total_amount, nominee_count, is_onekv)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
            "#,
        )
            .bind(event.validator_account_id.to_string())
            .bind(event.discovered_block_number as i64)
            .bind(event.nominator_stash_account_id.to_string())
            .bind(event.active_amount.to_string())
            .bind(event.total_amount.to_string())
            .bind(event.nominee_count as i64)
            .bind(event.is_onekv)
            .fetch_one(&self.connection_pool)
            .await?;
        Ok(result.0 as u32)
    }

    pub async fn save_nomination_amount_change_event(
        &self,
        event: &app_event::NominationAmountChange,
    ) -> anyhow::Result<u32> {
        self.save_account(&event.validator_account_id).await?;
        self.save_account(&event.nominator_stash_account_id).await?;
        let result: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO sub_app_event_nomination_amount_change (validator_account_id, discovered_block_number, nominator_stash_account_id, prev_active_amount, prev_total_amount, prev_nominee_count, active_amount, total_amount, nominee_count, is_onekv)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id
            "#,
        )
            .bind(event.validator_account_id.to_string())
            .bind(event.discovered_block_number as i64)
            .bind(event.nominator_stash_account_id.to_string())
            .bind(event.prev_active_amount.to_string())
            .bind(event.prev_total_amount.to_string())
            .bind(event.prev_nominee_count as i64)
            .bind(event.active_amount.to_string())
            .bind(event.total_amount.to_string())
            .bind(event.nominee_count as i64)
            .bind(event.is_onekv)
            .fetch_one(&self.connection_pool)
            .await?;
        Ok(result.0 as u32)
    }

    pub async fn save_active_next_session_event(
        &self,
        validator_account_id: &AccountId,
        discovered_block_number: u64,
    ) -> anyhow::Result<u32> {
        self.save_account(validator_account_id).await?;
        let result: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO sub_app_event_validator_active_next_session (validator_account_id, discovered_block_number)
            VALUES ($1, $2)
            RETURNING id
            "#,
        )
            .bind(validator_account_id.to_string())
            .bind(discovered_block_number as i64)
            .fetch_one(&self.connection_pool)
            .await?;
        Ok(result.0 as u32)
    }

    pub async fn save_inactive_next_session_event(
        &self,
        validator_account_id: &AccountId,
        discovered_block_number: u64,
    ) -> anyhow::Result<u32> {
        self.save_account(validator_account_id).await?;
        let result: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO sub_app_event_validator_inactive_next_session (validator_account_id, discovered_block_number)
            VALUES ($1, $2)
            RETURNING id
            "#,
        )
            .bind(validator_account_id.to_string())
            .bind(discovered_block_number as i64)
            .fetch_one(&self.connection_pool)
            .await?;
        Ok(result.0 as u32)
    }

    pub async fn save_active_event(
        &self,
        validator_account_id: &AccountId,
        discovered_block_number: u64,
    ) -> anyhow::Result<u32> {
        self.save_account(validator_account_id).await?;
        let result: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO sub_app_event_validator_active (validator_account_id, discovered_block_number)
            VALUES ($1, $2)
            RETURNING id
            "#,
        )
            .bind(validator_account_id.to_string())
            .bind(discovered_block_number as i64)
            .fetch_one(&self.connection_pool)
            .await?;
        Ok(result.0 as u32)
    }

    pub async fn save_inactive_event(
        &self,
        validator_account_id: &AccountId,
        discovered_block_number: u64,
    ) -> anyhow::Result<u32> {
        self.save_account(validator_account_id).await?;
        let result: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO sub_app_event_validator_inactive (validator_account_id, discovered_block_number)
            VALUES ($1, $2)
            RETURNING id
            "#,
        )
            .bind(validator_account_id.to_string())
            .bind(discovered_block_number as i64)
            .fetch_one(&self.connection_pool)
            .await?;
        Ok(result.0 as u32)
    }

    pub async fn save_session_keys_changed(
        &self,
        validator_account_id: &AccountId,
        session_keys: &str,
        discovered_block_number: u64,
    ) -> anyhow::Result<u32> {
        self.save_account(validator_account_id).await?;
        let result: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO sub_app_event_validator_session_keys_changed (validator_account_id, session_keys, discovered_block_number)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
        )
            .bind(validator_account_id.to_string())
            .bind(session_keys)
            .bind(discovered_block_number as i64)
            .fetch_one(&self.connection_pool)
            .await?;
        Ok(result.0 as u32)
    }

    pub async fn save_identity_changed(
        &self,
        validator_account_id: &AccountId,
        identity_display: &Option<String>,
        discovered_block_number: u64,
    ) -> anyhow::Result<u32> {
        self.save_account(validator_account_id).await?;
        let result: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO sub_app_event_validator_identity_changed (validator_account_id, identity_display, discovered_block_number)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
        )
            .bind(validator_account_id.to_string())
            .bind(identity_display)
            .bind(discovered_block_number as i64)
            .fetch_one(&self.connection_pool)
            .await?;
        Ok(result.0 as u32)
    }

    pub async fn save_onekv_binary_version_change_event(
        &self,
        validator_account_id: &AccountId,
        prev_version: &Option<String>,
        current_version: &Option<String>,
    ) -> anyhow::Result<u32> {
        self.save_account(validator_account_id).await?;
        let result: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO sub_app_event_onekv_binary_version_change (validator_account_id, prev_version, current_version)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
        )
            .bind(validator_account_id.to_string())
            .bind(prev_version)
            .bind(current_version)
            .fetch_one(&self.connection_pool)
            .await?;
        Ok(result.0 as u32)
    }

    pub async fn save_onekv_rank_change_event(
        &self,
        validator_account_id: &AccountId,
        prev_rank: Option<u64>,
        current_rank: Option<u64>,
    ) -> anyhow::Result<u32> {
        self.save_account(validator_account_id).await?;
        let result: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO sub_app_event_onekv_rank_change (validator_account_id, prev_rank, current_rank)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
        )
            .bind(validator_account_id.to_string())
            .bind(prev_rank.map(|rank| rank as i64))
            .bind(current_rank.map(|rank| rank as i64))
            .fetch_one(&self.connection_pool)
            .await?;
        Ok(result.0 as u32)
    }

    pub async fn save_onekv_location_change_event(
        &self,
        validator_account_id: &AccountId,
        prev_location: &Option<String>,
        current_location: &Option<String>,
    ) -> anyhow::Result<u32> {
        self.save_account(validator_account_id).await?;
        let result: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO sub_app_event_onekv_location_change (validator_account_id, prev_location, current_location)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
        )
            .bind(validator_account_id.to_string())
            .bind(prev_location)
            .bind(current_location)
            .fetch_one(&self.connection_pool)
            .await?;
        Ok(result.0 as u32)
    }

    pub async fn save_onekv_validity_change_event(
        &self,
        validator_account_id: &AccountId,
        is_valid: bool,
    ) -> anyhow::Result<u32> {
        self.save_account(validator_account_id).await?;
        let result: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO sub_app_event_onekv_validity_change (validator_account_id, is_valid)
            VALUES ($1, $2)
            RETURNING id
            "#,
        )
        .bind(validator_account_id.to_string())
        .bind(is_valid)
        .fetch_one(&self.connection_pool)
        .await?;
        Ok(result.0 as u32)
    }
}
