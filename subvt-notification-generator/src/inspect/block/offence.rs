use crate::{NotificationGenerator, CONFIG};
use std::sync::Arc;
use subvt_persistence::postgres::app::PostgreSQLAppStorage;
use subvt_persistence::postgres::network::PostgreSQLNetworkStorage;
use subvt_substrate_client::SubstrateClient;
use subvt_types::app::{Block, NotificationTypeCode};

impl NotificationGenerator {
    /// Checks validator offline events.
    pub(crate) async fn inspect_offline_offences(
        &self,
        network_postgres: Arc<PostgreSQLNetworkStorage>,
        app_postgres: Arc<PostgreSQLAppStorage>,
        substrate_client: Arc<SubstrateClient>,
        block: &Block,
    ) -> anyhow::Result<()> {
        log::debug!("Inspect block #{} for offline offences.", block.number);
        for event in network_postgres
            .get_validator_offline_events_in_block(&block.hash)
            .await?
        {
            let rules = app_postgres
                .get_notification_rules_for_validator(
                    &NotificationTypeCode::ChainValidatorOfflineOffence.to_string(),
                    CONFIG.substrate.network_id,
                    &event.validator_account_id,
                )
                .await?;
            self.generate_notifications(
                app_postgres.clone(),
                substrate_client.clone(),
                &rules,
                block.number,
                &event.validator_account_id,
                Some(&event.clone()),
            )
            .await?;
        }
        Ok(())
    }
}
