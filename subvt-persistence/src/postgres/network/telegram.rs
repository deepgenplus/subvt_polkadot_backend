//! Telegram-bot-related storage. Used by the `subvt-telegram-bot` crate.
use crate::postgres::network::PostgreSQLNetworkStorage;
use std::str::FromStr;
use subvt_types::crypto::AccountId;
use subvt_types::telegram::TelegramChatState;

impl PostgreSQLNetworkStorage {
    pub async fn get_chat_count(&self) -> anyhow::Result<u64> {
        let chat_count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(DISTINCT telegram_chat_id)
            FROM sub_telegram_chat
            WHERE deleted_at IS NULL
            "#,
        )
        .fetch_one(&self.connection_pool)
        .await?;
        Ok(chat_count.0 as u64)
    }

    pub async fn get_chat_validator_count(&self) -> anyhow::Result<u64> {
        let validator_count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(DISTINCT validator_account_id) FROM (
                SELECT validator_account_id
                FROM sub_telegram_chat_validator
                WHERE deleted_at IS NULL
            ) AS validators
            "#,
        )
        .fetch_one(&self.connection_pool)
        .await?;
        Ok(validator_count.0 as u64)
    }

    pub async fn get_chat_ids(&self) -> anyhow::Result<Vec<i64>> {
        let chat_validators: Vec<(i64,)> = sqlx::query_as(
            r#"
            SELECT telegram_chat_id
            FROM sub_telegram_chat
            WHERE deleted_at IS NULL
            "#,
        )
        .fetch_all(&self.connection_pool)
        .await?;
        Ok(chat_validators.iter().map(|v| v.0).collect())
    }

    pub async fn get_chat_validator_account_ids(
        &self,
        telegram_chat_id: i64,
    ) -> anyhow::Result<Vec<AccountId>> {
        let chat_validators: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT validator_account_id
            FROM sub_telegram_chat_validator
            WHERE telegram_chat_id = $1
            AND deleted_at IS NULL
            "#,
        )
        .bind(telegram_chat_id)
        .fetch_all(&self.connection_pool)
        .await?;
        let mut account_ids = Vec::new();
        for chat_validator in &chat_validators {
            account_ids.push(AccountId::from_str(&chat_validator.0)?);
        }
        Ok(account_ids)
    }

    pub async fn chat_exists_by_id(&self, telegram_chat_id: i64) -> anyhow::Result<bool> {
        let record_count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(DISTINCT telegram_chat_id) FROM sub_telegram_chat
            WHERE telegram_chat_id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(telegram_chat_id)
        .fetch_one(&self.connection_pool)
        .await?;
        Ok(record_count.0 > 0)
    }

    pub async fn save_chat(
        &self,
        app_user_id: u32,
        telegram_chat_id: i64,
        state: &TelegramChatState,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sub_telegram_chat (app_user_id, telegram_chat_id, state)
            VALUES ($1, $2, $3)
            ON CONFLICT(telegram_chat_id) DO UPDATE
            SET app_user_id = $1, deleted_at = NULL
            "#,
        )
        .bind(app_user_id as i32)
        .bind(telegram_chat_id)
        .bind(&state.to_string())
        .execute(&self.connection_pool)
        .await?;
        Ok(())
    }

    pub async fn get_chat_app_user_id(&self, telegram_chat_id: i64) -> anyhow::Result<u32> {
        let app_user_id: (i32,) = sqlx::query_as(
            r#"
            SELECT app_user_id FROM sub_telegram_chat
            WHERE telegram_chat_id = $1
            "#,
        )
        .bind(telegram_chat_id)
        .fetch_one(&self.connection_pool)
        .await?;
        Ok(app_user_id.0 as u32)
    }

    pub async fn chat_has_validator(
        &self,
        telegram_chat_id: i64,
        validator_account_id: &AccountId,
    ) -> anyhow::Result<bool> {
        let record_count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(DISTINCT id) FROM sub_telegram_chat_validator
            WHERE telegram_chat_id = $1 AND validator_account_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(telegram_chat_id)
        .bind(&validator_account_id.to_string())
        .fetch_one(&self.connection_pool)
        .await?;
        Ok(record_count.0 > 0)
    }

    pub async fn add_validator_to_chat(
        &self,
        telegram_chat_id: i64,
        validator_account_id: &AccountId,
    ) -> anyhow::Result<u64> {
        self.save_account(validator_account_id).await?;
        let id: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO sub_telegram_chat_validator (telegram_chat_id, validator_account_id)
            VALUES ($1, $2)
            ON CONFLICT(telegram_chat_id, validator_account_id) DO UPDATE SET deleted_at = NULL
            RETURNING id
            "#,
        )
        .bind(telegram_chat_id as i64)
        .bind(&validator_account_id.to_string())
        .fetch_one(&self.connection_pool)
        .await?;
        Ok(id.0 as u64)
    }

    pub async fn remove_validator_from_chat(
        &self,
        telegram_chat_id: i64,
        validator_account_id: &AccountId,
    ) -> anyhow::Result<bool> {
        let maybe_id: Option<(i32,)> = sqlx::query_as(
            r#"
            UPDATE sub_telegram_chat_validator
            SET deleted_at = now()
            WHERE telegram_chat_id = $1
            AND validator_account_id = $2
            RETURNING id
            "#,
        )
        .bind(telegram_chat_id as i64)
        .bind(validator_account_id.to_string())
        .fetch_optional(&self.connection_pool)
        .await?;
        Ok(maybe_id.is_some())
    }

    pub async fn set_chat_state(
        &self,
        telegram_chat_id: i64,
        state: TelegramChatState,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE sub_telegram_chat
            SET state = $1
            WHERE telegram_chat_id = $2
            "#,
        )
        .bind(&state.to_string())
        .bind(telegram_chat_id as i64)
        .execute(&self.connection_pool)
        .await?;
        Ok(())
    }

    pub async fn get_chat_state(
        &self,
        telegram_chat_id: i64,
    ) -> anyhow::Result<Option<TelegramChatState>> {
        let maybe_state_str: Option<(String,)> = sqlx::query_as(
            r#"
            SELECT state FROM sub_telegram_chat
            WHERE telegram_chat_id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(telegram_chat_id)
        .fetch_optional(&self.connection_pool)
        .await?;
        if let Some(state_str) = maybe_state_str {
            let state = TelegramChatState::from_str(&state_str.0)?;
            Ok(Some(state))
        } else {
            Ok(None)
        }
    }
}