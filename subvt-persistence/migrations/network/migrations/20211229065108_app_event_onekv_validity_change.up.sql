CREATE TABLE IF NOT EXISTS sub_app_event_onekv_validity_change
(
    id                      SERIAL PRIMARY KEY,
    validator_account_id    VARCHAR(66) NOT NULL,
    is_valid                boolean NOT NULL,
    created_at              TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT now()
);

ALTER TABLE sub_app_event_onekv_validity_change
    ADD CONSTRAINT sub_app_event_onekv_validity_change_fk_validator
    FOREIGN KEY (validator_account_id)
    REFERENCES sub_account (id)
        ON DELETE RESTRICT
        ON UPDATE CASCADE;