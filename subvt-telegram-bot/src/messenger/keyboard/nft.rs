use crate::query::QueryType;
use crate::{Messenger, Query};
use frankenstein::{InlineKeyboardButton, InlineKeyboardMarkup};
use itertools::Itertools;
use subvt_types::sub_id::NFTCollection;
use tera::Context;

impl Messenger {
    pub fn get_nft_collection_keyboard(
        &self,
        validator_id: u64,
        collection_page: &NFTCollection,
        page_index: usize,
        has_prev: bool,
        has_next: bool,
    ) -> anyhow::Result<InlineKeyboardMarkup> {
        let sorted_chain_keys = collection_page
            .keys()
            .sorted_by_key(|chain| chain.name())
            .collect_vec();
        let mut rows = vec![];
        for chain in sorted_chain_keys {
            if let Some(chain_collection) = collection_page.get(chain) {
                for nft in chain_collection {
                    rows.push(vec![InlineKeyboardButton {
                        text: format!(
                            "{} - {}",
                            chain.name(),
                            if let Some(name) = &nft.name {
                                name
                            } else {
                                &nft.id
                            }
                        ),
                        url: nft.url.clone(),
                        login_url: None,
                        callback_data: None,
                        web_app: None,
                        switch_inline_query: None,
                        switch_inline_query_current_chat: None,
                        callback_game: None,
                        pay: None,
                    }]);
                }
            }
        }
        if has_prev || has_next {
            let mut nav_rows = vec![];
            nav_rows.push(InlineKeyboardButton {
                text: if has_prev {
                    let mut context = Context::new();
                    context.insert("page_number", &(page_index));
                    self.renderer.render("prev_page.html", &context)?
                } else {
                    "•".to_string()
                },
                url: None,
                login_url: None,
                callback_data: if has_prev {
                    Some(serde_json::to_string(&Query {
                        query_type: QueryType::NFTs(page_index - 1, false),
                        parameter: Some(validator_id.to_string()),
                    })?)
                } else {
                    Some(serde_json::to_string(&Query {
                        query_type: QueryType::NoOp,
                        parameter: None,
                    })?)
                },
                web_app: None,
                switch_inline_query: None,
                switch_inline_query_current_chat: None,
                callback_game: None,
                pay: None,
            });
            nav_rows.push(InlineKeyboardButton {
                text: if has_next {
                    let mut context = Context::new();
                    context.insert("page_number", &(page_index + 2));
                    self.renderer.render("next_page.html", &context)?
                } else {
                    "•".to_string()
                },
                url: None,
                login_url: None,
                callback_data: if has_next {
                    Some(serde_json::to_string(&Query {
                        query_type: QueryType::NFTs(page_index + 1, false),
                        parameter: Some(validator_id.to_string()),
                    })?)
                } else {
                    Some(serde_json::to_string(&Query {
                        query_type: QueryType::NoOp,
                        parameter: None,
                    })?)
                },
                web_app: None,
                switch_inline_query: None,
                switch_inline_query_current_chat: None,
                callback_game: None,
                pay: None,
            });
            rows.push(nav_rows);
        }
        rows.push(self.get_settings_button("close.html", QueryType::Close)?);
        Ok(InlineKeyboardMarkup {
            inline_keyboard: rows,
        })
    }
}
