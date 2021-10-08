//! Subscribes to the live network status data on Redis and publishes the data
//! through WebSocket pub/sub.

use anyhow::Context;
use async_trait::async_trait;
use bus::Bus;
use jsonrpsee::ws_server::{RpcModule, WsServerBuilder, WsStopHandle};
use lazy_static::lazy_static;
use log::{debug, error, warn};
use redis::Connection;
use std::sync::{Arc, Mutex, RwLock};
use subvt_config::Config;
use subvt_service_common::Service;
use subvt_types::subvt::{LiveNetworkStatus, LiveNetworkStatusDiff, LiveNetworkStatusUpdate};
use tokio::task::JoinHandle;

lazy_static! {
    static ref CONFIG: Config = Config::default();
}

#[derive(Clone, Debug)]
pub enum BusEvent {
    NewBlock(Box<LiveNetworkStatusDiff>),
    Error,
}

#[derive(Default)]
pub struct LiveNetworkStatusPresenter;

impl LiveNetworkStatusPresenter {
    async fn read_current_network_status(
        connection: &mut Connection,
    ) -> anyhow::Result<LiveNetworkStatus> {
        let key = format!("subvt:{}:live_network_status", CONFIG.substrate.chain);
        let status_json_string: String = redis::cmd("GET")
            .arg(key)
            .query(connection)
            .context("Can't read network status from Redis.")?;
        let status: LiveNetworkStatus = serde_json::from_str(&status_json_string)
            .context("Can't deserialize network status json.")?;
        Ok(status)
    }

    async fn run_rpc_server(
        current_status: &Arc<RwLock<LiveNetworkStatus>>,
        bus: &Arc<Mutex<Bus<BusEvent>>>,
    ) -> anyhow::Result<(JoinHandle<()>, WsStopHandle)> {
        let rpc_ws_server = WsServerBuilder::default()
            .build(format!(
                "{}:{}",
                CONFIG.rpc.host, CONFIG.rpc.live_network_status_port
            ))
            .await?;
        let mut rpc_module = RpcModule::new(());
        let current_status = current_status.clone();
        let bus = bus.clone();
        rpc_module.register_subscription(
            "subscribe_live_network_status",
            "unsubscribe_live_network_status",
            move |_params, mut sink, _| {
                debug!("New subscription.");
                let mut bus_receiver = bus.lock().unwrap().add_rx();
                {
                    let current_status = current_status.read().unwrap();
                    if current_status.best_block_number != 0 {
                        let update = LiveNetworkStatusUpdate {
                            network: CONFIG.substrate.chain.clone(),
                            status: Some(current_status.clone()),
                            diff_base_block_number: None,
                            diff: None,
                        };
                        let _ = sink.send(&update);
                    }
                }
                std::thread::spawn(move || loop {
                    if let Ok(status_diff) = bus_receiver.recv() {
                        match status_diff {
                            BusEvent::NewBlock(status_diff) => {
                                let update = LiveNetworkStatusUpdate {
                                    network: CONFIG.substrate.chain.clone(),
                                    status: None,
                                    diff_base_block_number: None,
                                    diff: Some(*status_diff.clone()),
                                };
                                let send_result = sink.send(&update);
                                if let Err(error) = send_result {
                                    debug!("Subscription closed. {:?}", error);
                                    return;
                                } else {
                                    debug!("Published diff.");
                                }
                            }
                            BusEvent::Error => {
                                return;
                            }
                        }
                    }
                });
                Ok(())
            },
        )?;
        let stop_handle = rpc_ws_server.stop_handle();
        let join_handle = tokio::spawn(rpc_ws_server.start(rpc_module));
        Ok((join_handle, stop_handle))
    }
}

#[async_trait]
impl Service for LiveNetworkStatusPresenter {
    async fn run(&'static self) -> anyhow::Result<()> {
        let bus = Arc::new(Mutex::new(Bus::new(100)));
        let current_status = Arc::new(RwLock::new(LiveNetworkStatus::default()));
        let redis_client = redis::Client::open(CONFIG.redis.url.as_str()).context(format!(
            "Cannot connect to Redis at URL {}.",
            CONFIG.redis.url
        ))?;

        let mut pub_sub_connection = redis_client.get_connection()?;
        let mut pub_sub = pub_sub_connection.as_pubsub();
        pub_sub.subscribe(format!(
            "subvt:{}:live_network_status:publish:best_block_number",
            CONFIG.substrate.chain
        ))?;
        let mut data_connection = redis_client.get_connection()?;
        let (server_join_handle, server_stop_handle) =
            LiveNetworkStatusPresenter::run_rpc_server(&current_status, &bus).await?;

        let error: anyhow::Error = loop {
            let message = pub_sub.get_message();
            if let Err(error) = message {
                break error.into();
            }
            let payload = message.unwrap().get_payload();
            if let Err(error) = payload {
                break error.into();
            }
            let best_block_number: u64 = payload.unwrap();
            {
                let current_status = current_status.read().unwrap();
                if current_status.best_block_number == best_block_number {
                    warn!("Skip duplicate best block #{}.", best_block_number);
                    continue;
                }
            }
            debug!("New best block #{}.", best_block_number);
            match LiveNetworkStatusPresenter::read_current_network_status(&mut data_connection)
                .await
            {
                Ok(new_status) => {
                    {
                        let current_status = current_status.read().unwrap();
                        if current_status.best_block_number != 0 {
                            let diff = current_status.get_diff(&new_status);
                            let mut bus = bus.lock().unwrap();
                            bus.broadcast(BusEvent::NewBlock(Box::new(diff)));
                        }
                    }
                    let mut current_status = current_status.write().unwrap();
                    *current_status = new_status;
                }
                Err(error) => {
                    break error;
                }
            }
        };
        error!("{:?}", error);
        {
            let mut bus = bus.lock().unwrap();
            bus.broadcast(BusEvent::Error);
        }
        debug!("Stop RPC server.");
        server_stop_handle.clone().stop().await?;
        server_join_handle
            .await
            .expect("Server should be shut down.");
        debug!("RPC server stopped fully.");
        Err(error)
    }
}
