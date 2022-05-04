use crate::postgres::network::PostgreSQLNetworkStorage;

impl PostgreSQLNetworkStorage {
    pub async fn save_batch_interrupted_event(
        &self,
        block_hash: &str,
        extrinsic_index: Option<i32>,
        event_index: i32,
        item_index: i32,
        dispatch_error_debug: String,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sub_event_batch_interrupted (block_hash, extrinsic_index, event_index, item_index, dispatch_error_debug)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (block_hash, event_index) DO NOTHING
            "#)
            .bind(block_hash)
            .bind(extrinsic_index)
            .bind(event_index)
            .bind(item_index)
            .bind(dispatch_error_debug)
            .execute(&self.connection_pool)
            .await?;
        Ok(())
    }

    pub async fn update_batch_interrupted_event_batch_index(
        &self,
        block_hash: &str,
        batch_index: &Option<String>,
        event_index: i32,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE sub_event_batch_interrupted
            SET batch_index = $1
            WHERE block_hash = $2 AND event_index = $3
            "#,
        )
        .bind(batch_index)
        .bind(block_hash)
        .bind(event_index)
        .execute(&self.connection_pool)
        .await?;
        Ok(())
    }
}