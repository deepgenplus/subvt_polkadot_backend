//! Updates the complete 1KV data for the network on the database.

use async_trait::async_trait;
use lazy_static::lazy_static;
use log::{debug, error, info};
use subvt_config::Config;
use subvt_persistence::postgres::PostgreSQLStorage;
use subvt_service_common::Service;
use subvt_types::onekv::{Candidate, CandidateDetails};

lazy_static! {
    static ref CONFIG: Config = Config::default();
}

pub struct OneKVUpdater {
    http_client: reqwest::Client,
}

impl Default for OneKVUpdater {
    fn default() -> Self {
        let http_client: reqwest::Client = reqwest::Client::builder()
            .gzip(true)
            .brotli(true)
            .timeout(std::time::Duration::from_secs(
                CONFIG.onekv.request_timeout_seconds,
            ))
            .build()
            .unwrap();
        Self { http_client }
    }
}

impl OneKVUpdater {
    async fn update(&self, postgres: &PostgreSQLStorage) -> anyhow::Result<()> {
        info!("Update 1KV.");
        info!("Fetch candidate list.");
        let response = self
            .http_client
            .get(&CONFIG.onekv.candidate_list_endpoint)
            .send()
            .await?;
        let candidates: Vec<Candidate> = response.json().await?;
        info!(
            "Fetched {} candidates. Fetch candidate details.",
            candidates.len()
        );
        // get details for each candidate
        for (index, candidate) in candidates.iter().enumerate() {
            let response = self
                .http_client
                .get(&format!(
                    "{}{}",
                    CONFIG.onekv.candidate_details_endpoint, candidate.stash_address
                ))
                .send()
                .await?;
            let mut candidate_details: CandidateDetails = response.json().await?;
            candidate_details.score = candidate.score.clone();
            postgres.save_onekv_candidate(&candidate_details).await?;
            debug!(
                "Fetched and persisted candidate {} of {} :: {}.",
                index + 1,
                candidates.len(),
                candidate.stash_address
            );
        }
        info!("1KV update completed.");
        Ok(())
    }
}

#[async_trait]
impl Service for OneKVUpdater {
    async fn run(&'static self) -> anyhow::Result<()> {
        info!(
            "1KV updater has started with {} seconds refresh wait period.",
            CONFIG.onekv.refresh_seconds
        );
        let postgres = PostgreSQLStorage::new(&CONFIG).await?;
        loop {
            if let Err(error) = self.update(&postgres).await {
                error!("1KV update has failed: {:?}", error);
                error!("Will retry in {} seconds.", CONFIG.onekv.refresh_seconds);
            }
            std::thread::sleep(std::time::Duration::from_secs(CONFIG.onekv.refresh_seconds));
        }
    }
}
