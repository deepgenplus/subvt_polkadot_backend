//! Contains the logic to process new blocks' events and extrinsics and persist notifications
//! to be later sent by `subvt-notification-sender`.

use crate::{NotificationGenerator, CONFIG};
use async_lock::Mutex;
use once_cell::sync::OnceCell;
use std::sync::Arc;
use subvt_persistence::postgres::app::PostgreSQLAppStorage;
use subvt_persistence::postgres::network::PostgreSQLNetworkStorage;
use subvt_substrate_client::SubstrateClient;
use subvt_types::rdb::BlockProcessedNotification;

mod authorship;
mod chilling;
mod offence;
mod payout;
mod set_controller;
mod validate;

impl NotificationGenerator {
    async fn inspect_block(
        &self,
        network_postgres: Arc<PostgreSQLNetworkStorage>,
        app_postgres: Arc<PostgreSQLAppStorage>,
        substrate_client: Arc<SubstrateClient>,
        block_number: u64,
    ) -> anyhow::Result<()> {
        log::info!("Inspect block #{}.", block_number);
        let block = match network_postgres.get_block_by_number(block_number).await? {
            Some(block) => block,
            None => {
                log::error!("Block ${} not found.", block_number);
                return Ok(());
            }
        };
        self.inspect_block_authorship(app_postgres.clone(), substrate_client.clone(), &block)
            .await?;
        self.inspect_offline_offences(
            network_postgres.clone(),
            app_postgres.clone(),
            substrate_client.clone(),
            &block,
        )
        .await?;
        self.inspect_chillings(
            network_postgres.clone(),
            app_postgres.clone(),
            substrate_client.clone(),
            &block,
        )
        .await?;
        self.inspect_validate_extrinsics(
            network_postgres.clone(),
            app_postgres.clone(),
            substrate_client.clone(),
            &block,
        )
        .await?;
        self.inspect_set_controller_extrinsics(
            network_postgres.clone(),
            app_postgres.clone(),
            substrate_client.clone(),
            &block,
        )
        .await?;
        self.inspect_payout_stakers_extrinsics(
            network_postgres.clone(),
            app_postgres.clone(),
            substrate_client.clone(),
            &block,
        )
        .await?;

        network_postgres
            .save_notification_generator_state(&block.hash, block_number)
            .await?;
        log::info!("Completed the inspection of block #{}.", block_number);
        Ok(())
    }

    async fn on_new_block(
        &self,
        network_postgres: Arc<PostgreSQLNetworkStorage>,
        app_postgres: Arc<PostgreSQLAppStorage>,
        substrate_client: Arc<SubstrateClient>,
        last_processed_block_number_mutex: Arc<Mutex<Option<u64>>>,
        postgres_notification: BlockProcessedNotification,
    ) -> anyhow::Result<()> {
        let new_block_number = postgres_notification.block_number;
        log::info!("Inspect block #{}.", new_block_number);
        let mut maybe_last_processed_block_number = last_processed_block_number_mutex.lock().await;
        let start_block_number =
            if let Some(last_processed_block_number) = *maybe_last_processed_block_number {
                last_processed_block_number + 1
            } else {
                new_block_number
            };
        for block_number in start_block_number..=new_block_number {
            match self
                .inspect_block(
                    network_postgres.clone(),
                    app_postgres.clone(),
                    substrate_client.clone(),
                    block_number,
                )
                .await
            {
                Ok(()) => {
                    *maybe_last_processed_block_number = Some(block_number);
                }
                Err(error) => {
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn start_block_inspection(&'static self) -> anyhow::Result<()> {
        loop {
            log::info!("Start inspecting new blocks.");
            let network_postgres = Arc::new(
                PostgreSQLNetworkStorage::new(&CONFIG, CONFIG.get_network_postgres_url()).await?,
            );
            let app_postgres =
                Arc::new(PostgreSQLAppStorage::new(&CONFIG, CONFIG.get_app_postgres_url()).await?);
            let last_processed_block_number_mutex = Arc::new(Mutex::new(
                network_postgres
                    .get_notification_generator_state()
                    .await?
                    .map(|state| state.1),
            ));
            let substrate_client: Arc<SubstrateClient> =
                Arc::new(SubstrateClient::new(&CONFIG).await?);
            let error_cell: Arc<OnceCell<anyhow::Error>> = Arc::new(OnceCell::new());
            network_postgres
                .subscribe_to_processed_blocks(|notification| async {
                    let error_cell = error_cell.clone();
                    if let Some(error) = error_cell.get() {
                        return Err(anyhow::anyhow!("{:?}", error));
                    }
                    let last_processed_block_number_mutex =
                        last_processed_block_number_mutex.clone();
                    let network_postgres = network_postgres.clone();
                    let app_postgres = app_postgres.clone();
                    let substrate_client = substrate_client.clone();
                    tokio::spawn(async move {
                        if let Err(error) = self
                            .on_new_block(
                                network_postgres,
                                app_postgres,
                                substrate_client,
                                last_processed_block_number_mutex,
                                notification,
                            )
                            .await
                        {
                            log::error!("Error while processing block: {:?}.", error);
                            let _ = error_cell.set(error);
                        }
                    });
                    Ok(())
                })
                .await;
            let delay_seconds = CONFIG.common.recovery_retry_seconds;
            log::error!(
                "Block inspection exited. Will restart after {} seconds.",
                delay_seconds
            );
            std::thread::sleep(std::time::Duration::from_secs(delay_seconds));
        }
    }
}