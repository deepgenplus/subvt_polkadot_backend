use subvt_persistence::postgres::network::PostgreSQLNetworkStorage;
use subvt_types::app::event::democracy::{AccountVote, Conviction};
use subvt_types::substrate::event::DemocracyEvent;

pub(crate) async fn process_democracy_event(
    postgres: &PostgreSQLNetworkStorage,
    block_hash: &str,
    event_index: usize,
    event: &DemocracyEvent,
) -> anyhow::Result<()> {
    match event {
        DemocracyEvent::Cancelled {
            extrinsic_index,
            referendum_index,
        } => {
            let extrinsic_index = extrinsic_index.map(|extrinsic_index| extrinsic_index as i32);
            postgres
                .save_democracy_cancelled_event(
                    block_hash,
                    extrinsic_index,
                    event_index as i32,
                    *referendum_index,
                )
                .await?;
        }
        DemocracyEvent::Delegated {
            extrinsic_index,
            original_account_id,
            delegate_account_id,
        } => {
            let extrinsic_index = extrinsic_index.map(|extrinsic_index| extrinsic_index as i32);
            postgres
                .save_democracy_delegated_event(
                    block_hash,
                    extrinsic_index,
                    event_index as i32,
                    original_account_id,
                    delegate_account_id,
                )
                .await?;
        }
        DemocracyEvent::NotPassed {
            extrinsic_index,
            referendum_index,
        } => {
            let extrinsic_index = extrinsic_index.map(|extrinsic_index| extrinsic_index as i32);
            postgres
                .save_democracy_not_passed_event(
                    block_hash,
                    extrinsic_index,
                    event_index as i32,
                    *referendum_index,
                )
                .await?;
        }
        DemocracyEvent::Passed {
            extrinsic_index,
            referendum_index,
        } => {
            let extrinsic_index = extrinsic_index.map(|extrinsic_index| extrinsic_index as i32);
            postgres
                .save_democracy_passed_event(
                    block_hash,
                    extrinsic_index,
                    event_index as i32,
                    *referendum_index,
                )
                .await?;
        }
        DemocracyEvent::Proposed {
            extrinsic_index,
            proposal_index,
            deposit,
        } => {
            let extrinsic_index = extrinsic_index.map(|extrinsic_index| extrinsic_index as i32);
            postgres
                .save_democracy_proposed_event(
                    block_hash,
                    extrinsic_index,
                    event_index as i32,
                    *proposal_index,
                    *deposit,
                )
                .await?;
        }
        DemocracyEvent::Seconded {
            extrinsic_index,
            account_id,
            proposal_index,
        } => {
            let extrinsic_index = extrinsic_index.map(|extrinsic_index| extrinsic_index as i32);
            postgres
                .save_democracy_seconded_event(
                    block_hash,
                    extrinsic_index,
                    event_index as i32,
                    account_id,
                    *proposal_index,
                )
                .await?;
        }
        DemocracyEvent::Started {
            extrinsic_index,
            referendum_index,
            vote_threshold,
        } => {
            let extrinsic_index = extrinsic_index.map(|extrinsic_index| extrinsic_index as i32);
            postgres
                .save_democracy_started_event(
                    block_hash,
                    extrinsic_index,
                    event_index as i32,
                    *referendum_index,
                    &serde_json::to_string(vote_threshold)?,
                )
                .await?;
        }
        DemocracyEvent::Undelegated {
            extrinsic_index,
            account_id,
        } => {
            let extrinsic_index = extrinsic_index.map(|extrinsic_index| extrinsic_index as i32);
            postgres
                .save_democracy_undelegated_event(
                    block_hash,
                    extrinsic_index,
                    event_index as i32,
                    account_id,
                )
                .await?;
        }
        DemocracyEvent::Voted {
            extrinsic_index,
            account_id,
            referendum_index,
            vote,
        } => {
            let extrinsic_index = extrinsic_index.map(|extrinsic_index| extrinsic_index as i32);
            postgres
                .save_democracy_voted_event(
                    block_hash,
                    extrinsic_index,
                    event_index as i32,
                    account_id,
                    *referendum_index,
                    match vote {
                        AccountVote::Standard { vote, balance } => {
                            if vote.aye {
                                Some(*balance)
                            } else {
                                None
                            }
                        }
                        AccountVote::Split { aye, .. } => Some(*aye),
                    },
                    match vote {
                        AccountVote::Standard { vote, balance } => {
                            if !vote.aye {
                                Some(*balance)
                            } else {
                                None
                            }
                        }
                        AccountVote::Split { nay, .. } => Some(*nay),
                    },
                    match vote {
                        AccountVote::Standard { vote, .. } => match vote.conviction {
                            Conviction::None => Some(0),
                            Conviction::Locked1x => Some(1),
                            Conviction::Locked2x => Some(2),
                            Conviction::Locked3x => Some(3),
                            Conviction::Locked4x => Some(4),
                            Conviction::Locked5x => Some(5),
                            Conviction::Locked6x => Some(6),
                        },
                        AccountVote::Split { .. } => None,
                    },
                )
                .await?;
        }
    }
    Ok(())
}
