CREATE TABLE IF NOT EXISTS sub_app_event_removed_validator
(
    id                      SERIAL PRIMARY KEY,
    validator_account_id    VARCHAR(66) NOT NULL,
    discovered_block_number bigint NOT NULL,
    created_at              TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT now()
);

ALTER TABLE sub_app_event_removed_validator
    ADD CONSTRAINT sub_app_event_removed_validator_fk_validator_account_id
    FOREIGN KEY (validator_account_id)
        REFERENCES sub_account (id)
        ON DELETE RESTRICT
        ON UPDATE CASCADE;