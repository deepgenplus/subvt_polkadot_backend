//! `/networkstatus` command processor.
use crate::{MessageType, TelegramBot};

impl TelegramBot {
    //! Fetches the current live network status from the network Redis instance and
    //! sends it to the chat.
    pub(crate) async fn process_network_status_command(&self, chat_id: i64) -> anyhow::Result<()> {
        self.messenger
            .send_message(
                &self.app_postgres,
                &self.network_postgres,
                chat_id,
                Box::new(MessageType::NetworkStatus(
                    self.redis.get_current_network_status().await?,
                )),
            )
            .await?;
        Ok(())
    }
}
