CREATE TABLE IF NOT EXISTS sub_event_democracy_voted
(
    id                  SERIAL PRIMARY KEY,
    block_hash          VARCHAR(66) NOT NULL,
    extrinsic_index     integer,
    event_index         integer NOT NULL,
    account_id          VARCHAR(66) NOT NULL,
    referendum_index    bigint NOT NULL,
    vote_encoded_hex    text NOT NULL,
    created_at          TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT now()
);

ALTER TABLE sub_event_democracy_voted
    ADD CONSTRAINT sub_event_democracy_voted_fk_block
    FOREIGN KEY (block_hash)
        REFERENCES sub_block (hash)
        ON DELETE CASCADE
        ON UPDATE CASCADE;

ALTER TABLE sub_event_democracy_voted
    ADD CONSTRAINT sub_event_democracy_voted_fk_account_id
    FOREIGN KEY (account_id)
        REFERENCES sub_account (id)
        ON DELETE RESTRICT
        ON UPDATE CASCADE;

CREATE INDEX sub_event_democracy_voted_idx_block_hash
    ON sub_event_democracy_voted (block_hash);
CREATE INDEX sub_event_democracy_voted_idx_referendum_index
    ON sub_event_democracy_voted (referendum_index);
CREATE INDEX sub_event_democracy_voted_idx_account_id
    ON sub_event_democracy_voted (account_id);