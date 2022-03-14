use crate::{NotificationGenerator, CONFIG};
use std::sync::Arc;
use subvt_persistence::postgres::app::PostgreSQLAppStorage;
use subvt_persistence::postgres::network::PostgreSQLNetworkStorage;
use subvt_substrate_client::SubstrateClient;
use subvt_types::app::app_event;
use subvt_types::app::NotificationTypeCode;
use subvt_types::subvt::ValidatorDetails;

impl NotificationGenerator {
    pub(crate) async fn inspect_onekv_location_change(
        &self,
        network_postgres: Arc<PostgreSQLNetworkStorage>,
        app_postgres: Arc<PostgreSQLAppStorage>,
        substrate_client: Arc<SubstrateClient>,
        finalized_block_number: u64,
        last: &ValidatorDetails,
        current: &ValidatorDetails,
    ) -> anyhow::Result<()> {
        if current.onekv_location != last.onekv_location {
            log::debug!(
                "1KV location of {} changed from {:?} to {:?}.",
                current.account.address,
                last.onekv_location,
                current.onekv_location,
            );
            let rules = app_postgres
                .get_notification_rules_for_validator(
                    &NotificationTypeCode::OneKVValidatorLocationChange.to_string(),
                    CONFIG.substrate.network_id,
                    &current.account.id,
                )
                .await?;
            self.generate_notifications(
                app_postgres,
                substrate_client,
                &rules,
                finalized_block_number,
                &current.account.id,
                Some(&app_event::OneKVLocationChange {
                    validator_account_id: current.account.id.clone(),
                    prev_location: last.onekv_location.clone(),
                    current_location: current.onekv_location.clone(),
                }),
            )
            .await?;
            network_postgres
                .save_onekv_location_change_event(
                    &current.account.id,
                    &last.onekv_location,
                    &current.onekv_location,
                )
                .await?;
        }
        Ok(())
    }
}