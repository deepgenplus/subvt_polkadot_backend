use crate::NotificationGenerator;
use async_lock::Mutex;
use log::{error, info};
use std::sync::Arc;
use subvt_config::Config;
use subvt_persistence::postgres::app::PostgreSQLAppStorage;
use subvt_persistence::postgres::network::PostgreSQLNetworkStorage;
use subvt_types::app::{Block, Notification, NotificationTypeCode, UserNotificationRule};
use subvt_types::crypto::AccountId;

impl NotificationGenerator {
    async fn generate_notifications(
        config: &Config,
        app_postgres: &Arc<PostgreSQLAppStorage>,
        block: &Block,
        (extrinsic_index, event_index): (Option<u32>, Option<u32>),
        rules: &[UserNotificationRule],
        validator_account_id: &AccountId,
    ) -> anyhow::Result<()> {
        for rule in rules {
            println!(
                "Generate {} notification for {} in block #{}.",
                block.number, rule.notification_type.code, validator_account_id,
            );
            for channel in &rule.notification_channels {
                let notification = Notification {
                    id: 0,
                    user_id: rule.user_id,
                    user_notification_rule_id: rule.id,
                    network_id: config.substrate.network_id,
                    period_type: rule.period_type.clone(),
                    period: rule.period,
                    validator_account_id: validator_account_id.clone(),
                    notification_type_code: rule.notification_type.code.clone(),
                    parameter_type_id: None,
                    parameter_value: None,
                    block_hash: Some(block.hash.clone()),
                    block_number: Some(block.number),
                    block_timestamp: block.timestamp,
                    extrinsic_index,
                    event_index,
                    user_notification_channel_id: channel.id,
                    notification_channel_code: channel.channel_code.clone(),
                    notification_target: channel.target.clone(),
                    log: None,
                    created_at: None,
                    sent_at: None,
                    delivered_at: None,
                    read_at: None,
                };
                let _ = app_postgres.save_notification(&notification).await?;
            }
        }
        Ok(())
    }

    async fn process_block_authorship(
        config: &Config,
        app_postgres: &Arc<PostgreSQLAppStorage>,
        block: &Block,
    ) -> anyhow::Result<()> {
        let validator_account_id = if let Some(author_account_id) = &block.author_account_id {
            author_account_id
        } else {
            error!("Block ${} author is null.", block.number);
            return Ok(());
        };
        let rules = app_postgres
            .get_notification_rules_for_validator(
                &NotificationTypeCode::ChainValidatorBlockAuthorship.to_string(),
                config.substrate.network_id,
                validator_account_id,
            )
            .await?;
        NotificationGenerator::generate_notifications(
            config,
            app_postgres,
            block,
            (None, None),
            &rules,
            validator_account_id,
        )
        .await?;
        Ok(())
    }

    async fn process_offline_offences(
        config: &Config,
        app_postgres: &Arc<PostgreSQLAppStorage>,
        network_postgres: &Arc<PostgreSQLNetworkStorage>,
        block: &Block,
    ) -> anyhow::Result<()> {
        for event in network_postgres
            .get_validator_offline_events_in_block(&block.hash)
            .await?
        {
            let rules = app_postgres
                .get_notification_rules_for_validator(
                    &NotificationTypeCode::ChainValidatorOfflineOffence.to_string(),
                    config.substrate.network_id,
                    &event.validator_account_id,
                )
                .await?;
            NotificationGenerator::generate_notifications(
                config,
                app_postgres,
                block,
                (None, event.event_index),
                &rules,
                &event.validator_account_id,
            )
            .await?;
        }
        Ok(())
    }

    async fn process_chillings(
        config: &Config,
        app_postgres: &Arc<PostgreSQLAppStorage>,
        network_postgres: &Arc<PostgreSQLNetworkStorage>,
        block: &Block,
    ) -> anyhow::Result<()> {
        for event in network_postgres
            .get_chilled_events_in_block(&block.hash)
            .await?
        {
            let rules = app_postgres
                .get_notification_rules_for_validator(
                    &NotificationTypeCode::ChainValidatorChilled.to_string(),
                    config.substrate.network_id,
                    &event.stash_account_id,
                )
                .await?;
            NotificationGenerator::generate_notifications(
                config,
                app_postgres,
                block,
                (event.extrinsic_index, Some(event.event_index)),
                &rules,
                &event.stash_account_id,
            )
            .await?;
        }
        Ok(())
    }

    async fn process_validate_extrinsics(
        config: &Config,
        app_postgres: &Arc<PostgreSQLAppStorage>,
        network_postgres: &Arc<PostgreSQLNetworkStorage>,
        block: &Block,
    ) -> anyhow::Result<()> {
        for extrinsic in network_postgres
            .get_validate_extrinsics_in_block(&block.hash)
            .await?
        {
            let rules = app_postgres
                .get_notification_rules_for_validator(
                    &NotificationTypeCode::ChainValidateExtrinsic.to_string(),
                    config.substrate.network_id,
                    &extrinsic.stash_account_id,
                )
                .await?;
            NotificationGenerator::generate_notifications(
                config,
                app_postgres,
                block,
                (Some(extrinsic.extrinsic_index), None),
                &rules,
                &extrinsic.stash_account_id,
            )
            .await?;
        }
        Ok(())
    }

    async fn process_block(
        config: &Config,
        app_postgres: &Arc<PostgreSQLAppStorage>,
        network_postgres: &Arc<PostgreSQLNetworkStorage>,
        block_number: u64,
    ) -> anyhow::Result<()> {
        info!("Process block #{}.", block_number);
        let block = match network_postgres.get_block_by_number(block_number).await? {
            Some(block) => block,
            None => {
                error!("Block ${} not found.", block_number);
                return Ok(());
            }
        };
        NotificationGenerator::process_block_authorship(config, app_postgres, &block).await?;
        NotificationGenerator::process_offline_offences(
            config,
            app_postgres,
            network_postgres,
            &block,
        )
        .await?;
        NotificationGenerator::process_chillings(config, app_postgres, network_postgres, &block)
            .await?;
        NotificationGenerator::process_validate_extrinsics(
            config,
            app_postgres,
            network_postgres,
            &block,
        )
        .await?;

        network_postgres
            .save_notification_generator_state(&block.hash, block_number)
            .await
    }

    pub async fn start_processing_blocks(config: &'static Config) -> anyhow::Result<()> {
        let app_postgres =
            Arc::new(PostgreSQLAppStorage::new(config, config.get_app_postgres_url()).await?);
        let network_postgres = Arc::new(
            PostgreSQLNetworkStorage::new(config, config.get_network_postgres_url()).await?,
        );
        let maybe_last_processed_block_number_mutex = Arc::new(Mutex::new(
            network_postgres
                .get_notification_generator_state()
                .await?
                .map(|state| state.1),
        ));
        network_postgres
            .subscribe_to_processed_blocks(|notification| {
                let app_postgres = app_postgres.clone();
                let network_postgres = network_postgres.clone();
                let maybe_last_processed_block_number_mutex =
                    maybe_last_processed_block_number_mutex.clone();
                tokio::spawn(async move {
                    let mut maybe_block_number =
                        maybe_last_processed_block_number_mutex.lock().await;
                    let start_block_number = if let Some(block_number) = *maybe_block_number {
                        block_number + 1
                    } else {
                        notification.block_number
                    };

                    for block_number in start_block_number..=notification.block_number {
                        // process all, update last processed & database
                        match NotificationGenerator::process_block(
                            config,
                            &app_postgres,
                            &network_postgres,
                            block_number,
                        )
                        .await
                        {
                            Ok(()) => {
                                // update database
                                *maybe_block_number = Some(block_number);
                            }
                            Err(error) => {
                                error!(
                                    "Error while processing block #{}: {:?}",
                                    block_number, error
                                );
                            }
                        }
                    }
                });
            })
            .await?;
        Ok(())
    }
}