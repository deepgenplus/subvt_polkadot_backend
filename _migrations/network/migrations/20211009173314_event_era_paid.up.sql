CREATE TABLE IF NOT EXISTS sub_event_era_paid
(
    id                  SERIAL PRIMARY KEY,
    block_hash          VARCHAR(66) NOT NULL,
    extrinsic_index     integer,
    event_index         integer NOT NULL,
    era_index           bigint NOT NULL,
    validator_payout    VARCHAR(128) NOT NULL,
    remainder           VARCHAR(128) NOT NULL,
    created_at          TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT now()
);

ALTER TABLE sub_event_era_paid
    ADD CONSTRAINT sub_event_era_paid_fk_block
    FOREIGN KEY (block_hash)
        REFERENCES sub_block (hash)
        ON DELETE CASCADE
        ON UPDATE CASCADE;
ALTER TABLE sub_event_era_paid
    ADD CONSTRAINT sub_event_era_paid_fk_era
    FOREIGN KEY (era_index)
        REFERENCES sub_era (index)
        ON DELETE RESTRICT
        ON UPDATE CASCADE;