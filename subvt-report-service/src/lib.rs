//!  Public reporting REST services.
#![warn(clippy::disallowed_types)]
use actix_web::dev::Service as _;
use actix_web::{web, App, HttpResponse, HttpServer};
use anyhow::Context;
use async_trait::async_trait;
use futures_util::future::FutureExt;
use futures_util::StreamExt as _;
use lazy_static::lazy_static;
use std::sync::{Arc, RwLock};
use subvt_config::Config;
use subvt_persistence::postgres::network::PostgreSQLNetworkStorage;
use subvt_persistence::redis::Redis;
use subvt_service_common::{err::InternalServerError, Service};
use subvt_types::report::BlockSummary;
use subvt_types::subvt::ValidatorSummary;

mod era;
mod metrics;
mod onekv;
mod session;
mod validator;

lazy_static! {
    static ref CONFIG: Config = Config::default();
}

pub(crate) type ResultResponse = Result<HttpResponse, InternalServerError>;

#[derive(Clone)]
pub(crate) struct ServiceState {
    postgres: Arc<PostgreSQLNetworkStorage>,
    redis: Arc<Redis>,
    finalized_block_summary: Arc<RwLock<BlockSummary>>,
    active_validator_list: Arc<RwLock<Vec<ValidatorSummary>>>,
    inactive_validator_list: Arc<RwLock<Vec<ValidatorSummary>>>,
}

async fn on_server_ready() {
    log::info!("HTTP service started.");
}

#[derive(Default)]
pub struct ReportService;

#[async_trait(?Send)]
impl Service for ReportService {
    fn get_metrics_server_addr() -> (&'static str, u16) {
        (
            CONFIG.metrics.host.as_str(),
            CONFIG.metrics.report_service_port,
        )
    }

    async fn run(&'static self) -> anyhow::Result<()> {
        let postgres = Arc::new(
            PostgreSQLNetworkStorage::new(&CONFIG, CONFIG.get_network_postgres_url()).await?,
        );
        let redis = Arc::new(Redis::new()?);
        let finalized_block_summary = Arc::new(RwLock::new(BlockSummary::default()));
        let active_validator_list = Arc::new(RwLock::new(Vec::new()));
        let inactive_validator_list = Arc::new(RwLock::new(Vec::new()));

        let redis_client = redis::Client::open(CONFIG.redis.url.as_str()).context(format!(
            "Cannot connect to Redis at URL {}.",
            CONFIG.redis.url
        ))?;
        let mut pubsub_connection = redis_client.get_async_connection().await?.into_pubsub();
        pubsub_connection
            .subscribe(format!(
                "subvt:{}:validators:publish:finalized_block_number",
                CONFIG.substrate.chain
            ))
            .await?;
        let updater_finalized_block_summary = finalized_block_summary.clone();
        let updater_active_validator_list = active_validator_list.clone();
        let updater_inactive_validator_list = inactive_validator_list.clone();
        let updater_redis = Redis::new()?;
        tokio::spawn(async move {
            let mut pubsub_stream = pubsub_connection.on_message();
            let mut last_finalized_block_number = 0;

            loop {
                let _ = pubsub_stream.next().await;
                let finalized_block_summary =
                    match updater_redis.get_finalized_block_summary().await {
                        Ok(block_summary) => block_summary,
                        Err(error) => {
                            log::error!("{:?}", error);
                            continue;
                        }
                    };
                let finalized_block_number: u64 = finalized_block_summary.number;
                if last_finalized_block_number == finalized_block_number {
                    log::warn!(
                        "Skip duplicate finalized block #{}.",
                        finalized_block_number
                    );
                    continue;
                }
                log::info!("New finalized block #{}.", finalized_block_number);
                // finalized block
                {
                    match updater_finalized_block_summary.write() {
                        Ok(mut lock) => (*lock) = finalized_block_summary,
                        Err(error) => {
                            log::error!(
                                "Cannot get write lock for finalized block summary: {}",
                                error
                            );
                            continue;
                        }
                    }
                }
                // active list
                {
                    let current_active_validator_list = match updater_redis
                        .get_validator_list(finalized_block_number, true)
                        .await
                    {
                        Ok(list) => list,
                        Err(error) => {
                            log::error!("Cannot get active validator list from Redis: {}", error);
                            continue;
                        }
                    };
                    match updater_active_validator_list.write() {
                        Ok(mut lock) => (*lock) = current_active_validator_list,
                        Err(error) => {
                            log::error!(
                                "Cannot get write lock for active validator list: {}",
                                error
                            );
                            continue;
                        }
                    }
                }
                // inactive list
                {
                    let current_inactive_validator_list = match updater_redis
                        .get_validator_list(finalized_block_number, false)
                        .await
                    {
                        Ok(list) => list,
                        Err(error) => {
                            log::error!("Cannot get inactive validator list from Redis: {}", error);
                            continue;
                        }
                    };
                    match updater_inactive_validator_list.write() {
                        Ok(mut lock) => (*lock) = current_inactive_validator_list,
                        Err(error) => {
                            log::error!(
                                "Cannot get write lock for inactive validator list: {}",
                                error
                            );
                            continue;
                        }
                    }
                }
                last_finalized_block_number = finalized_block_number;
            }
        });

        log::info!("Starting HTTP service.");
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(ServiceState {
                    postgres: postgres.clone(),
                    redis: redis.clone(),
                    finalized_block_summary: finalized_block_summary.clone(),
                    active_validator_list: active_validator_list.clone(),
                    inactive_validator_list: inactive_validator_list.clone(),
                }))
                .wrap_fn(|request, service| {
                    metrics::request_counter().inc();
                    metrics::connection_count().inc();
                    let start = std::time::Instant::now();
                    service.call(request).map(move |result| {
                        match &result {
                            Ok(response) => {
                                let status_code = response.response().status();
                                metrics::response_time_ms()
                                    .observe(start.elapsed().as_millis() as f64);
                                metrics::response_status_code_counter(status_code.as_str()).inc();
                            }
                            Err(error) => {
                                let status_code = error.as_response_error().status_code();
                                metrics::response_time_ms()
                                    .observe(start.elapsed().as_millis() as f64);
                                metrics::response_status_code_counter(status_code.as_str()).inc();
                            }
                        }
                        metrics::connection_count().dec();
                        result
                    })
                })
                .service(era::era_validator_report_service)
                .service(era::era_active_validator_list_report_service)
                .service(era::era_inactive_validator_list_report_service)
                .service(era::era_report_service)
                .service(onekv::get_onekv_nominator_summaries)
                .service(session::validator::session_validator_report_service)
                .service(session::validator::session_validator_para_vote_service)
                .service(session::para::session_paras_vote_summaries_service)
                .service(validator::validator_summary_service)
                .service(validator::validator_details_service)
                .service(validator::validator_list_service)
                .service(validator::active_validator_list_service)
                .service(validator::inactive_validator_list_service)
                .service(validator::validator_search_service)
        })
        .workers(10)
        .disable_signals()
        .bind(format!(
            "{}:{}",
            CONFIG.http.service_host, CONFIG.http.report_service_port,
        ))?
        .run();
        let (server_result, _) = tokio::join!(server, on_server_ready());
        Ok(server_result?)
    }
}
