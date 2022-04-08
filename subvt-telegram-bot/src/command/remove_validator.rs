use crate::query::QueryType;
use crate::{Query, TelegramBot};

impl TelegramBot {
    pub(crate) async fn process_remove_validator_command(
        &self,
        chat_id: i64,
        args: &[String],
    ) -> anyhow::Result<()> {
        if let Some(validator_address) = args.get(0) {
            if let Some(chat_validator) = self
                .network_postgres
                .get_chat_validator_by_address(chat_id, validator_address)
                .await?
            {
                self.process_query(
                    chat_id,
                    None,
                    &Query {
                        query_type: QueryType::RemoveValidator,
                        parameter: Some(chat_validator.id.to_string()),
                    },
                )
                .await?;
            } else {
                self.process_validators_command(chat_id, QueryType::RemoveValidator)
                    .await?;
            }
        } else {
            self.process_validators_command(chat_id, QueryType::RemoveValidator)
                .await?;
        }
        Ok(())
    }
}
