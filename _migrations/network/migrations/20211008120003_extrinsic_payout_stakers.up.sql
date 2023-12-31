CREATE TABLE IF NOT EXISTS sub_extrinsic_payout_stakers
(
    id                      SERIAL PRIMARY KEY,
    block_hash              VARCHAR(66) NOT NULL,
    extrinsic_index         INTEGER NOT NULL,
    is_nested_call          boolean NOT NULL,
    nesting_index           text,
    caller_account_id       VARCHAR(66) NOT NULL,
    validator_account_id    VARCHAR(66) NOT NULL,
    era_index               bigint NOT NULL,
    is_successful           boolean NOT NULL,
    created_at              TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT now(),
    CONSTRAINT sub_extrinsic_payout_stakers_fk_block
        FOREIGN KEY (block_hash)
            REFERENCES sub_block (hash)
            ON DELETE CASCADE
            ON UPDATE CASCADE,
    CONSTRAINT sub_extrinsic_payout_stakers_fk_caller_account_id
        FOREIGN KEY (caller_account_id)
            REFERENCES sub_account (id)
            ON DELETE RESTRICT
            ON UPDATE CASCADE,
    CONSTRAINT sub_extrinsic_payout_stakers_fk_validator_account_id
        FOREIGN KEY (validator_account_id)
            REFERENCES sub_account (id)
            ON DELETE RESTRICT
            ON UPDATE CASCADE,
    CONSTRAINT sub_extrinsic_payout_stakers_fk_era
        FOREIGN KEY (era_index)
            REFERENCES sub_era (index)
            ON DELETE RESTRICT
            ON UPDATE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS sub_extrinsic_payout_stakers_u_extrinsic
    ON sub_extrinsic_payout_stakers (block_hash, extrinsic_index)
    WHERE nesting_index IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS sub_extrinsic_payout_stakers_u_extrinsic_nesting_index
    ON sub_extrinsic_payout_stakers (block_hash, extrinsic_index, nesting_index)
    WHERE nesting_index IS NOT NULL;

CREATE INDEX IF NOT EXISTS sub_extrinsic_payout_stakers_idx_block_hash
    ON sub_extrinsic_payout_stakers (block_hash);
CREATE INDEX IF NOT EXISTS sub_extrinsic_payout_stakers_idx_caller_account_id
    ON sub_extrinsic_payout_stakers (caller_account_id);
CREATE INDEX IF NOT EXISTS sub_extrinsic_payout_stakers_idx_validator_account_id
    ON sub_extrinsic_payout_stakers (validator_account_id);
CREATE INDEX IF NOT EXISTS sub_extrinsic_payout_stakers_idx_era_index
    ON sub_extrinsic_payout_stakers (era_index);
CREATE INDEX IF NOT EXISTS sub_extrinsic_payout_stakers_idx_is_successful
    ON sub_extrinsic_payout_stakers (is_successful);
CREATE INDEX IF NOT EXISTS sub_extrinsic_payout_stakers_idx_validator_era_successful
    ON sub_extrinsic_payout_stakers (validator_account_id, era_index, is_successful);
CREATE INDEX IF NOT EXISTS sub_extrinsic_payout_stakers_idx_era_block_success
    ON sub_extrinsic_payout_stakers (era_index, extrinsic_index, block_hash, is_successful);