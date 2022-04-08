use super::MessageType;
use crate::CONFIG;
use subvt_utility::text::get_condensed_address;
use tera::{Context, Tera};

mod network_status;
mod nomination_details;
mod nomination_summary;
mod validator_info;

impl MessageType {
    pub fn get_content(&self, renderer: &Tera) -> String {
        let mut context = Context::new();
        let template_name = match self {
            Self::Intro => "introduction.html",
            Self::Ok => "ok.html",
            Self::BadRequest => "bad_request.html",
            Self::GenericError => "generic_error.html",
            Self::Broadcast => "broadcast.html",
            Self::BroadcastConfirm => "broadcast_confirm.html",
            Self::UnknownCommand(command) => {
                context.insert("command", command);
                "unknown_command.html"
            }
            Self::InvalidAddress(address) => {
                context.insert("address", address);
                "invalid_address.html"
            }
            Self::InvalidAddressTryAgain(address) => {
                context.insert("address", address);
                "invalid_address_try_again.html"
            }
            Self::ValidatorNotFound { maybe_address } => {
                if let Some(address) = maybe_address {
                    context.insert("condensed_address", &get_condensed_address(address, None));
                }
                "validator_not_found.html"
            }
            Self::AddValidatorNotFound(address) => {
                context.insert("condensed_address", &get_condensed_address(address, None));
                "add_validator_not_found.html"
            }
            Self::ValidatorExistsOnChat(validator_display) => {
                context.insert("validator_display", validator_display);
                "validator_exists_on_chat.html"
            }
            Self::TooManyValidatorsOnChat => {
                context.insert(
                    "max_validators_per_chat",
                    &CONFIG.telegram_bot.max_validators_per_chat,
                );
                "too_many_validators_on_chat.html"
            }
            Self::NoValidatorsOnChat => "no_validators_on_chat.html",
            Self::ValidatorAdded => "validator_added.html",
            Self::AddValidator => "add_validator.html",
            Self::ValidatorList { .. } => "select_validator.html",
            Self::ValidatorInfo {
                address,
                maybe_validator_details,
                maybe_onekv_candidate_summary,
            } => {
                self.fill_validator_info_context(
                    &mut context,
                    address,
                    maybe_validator_details,
                    maybe_onekv_candidate_summary,
                );
                "validator_info.html"
            }
            Self::NominationSummary {
                validator_details, ..
            } => {
                self.fill_nomination_summary_context(&mut context, validator_details);
                "nomination_summary.html"
            }
            Self::NominationDetails {
                validator_details,
                onekv_nominator_account_ids,
            } => {
                self.fill_nomination_details_context(
                    &mut context,
                    validator_details,
                    onekv_nominator_account_ids,
                );
                "nomination_details.html"
            }
            Self::ValidatorRemoved(validator) => {
                let display = if let Some(display) = &validator.display {
                    display.clone()
                } else {
                    get_condensed_address(&validator.address, None)
                };
                context.insert("display", &display);
                "validator_removed.html"
            }
            Self::Settings => "settings_prompt.html",
            Self::NetworkStatus(network_status) => {
                self.fill_network_status_context(&mut context, network_status);
                "network_status.html"
            }
        };
        renderer.render(template_name, &context).unwrap()
    }
}
