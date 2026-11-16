-- 003_optimizations_and_constraints.sql
-- Performance indexes and case-insensitive uniqueness constraints

CREATE UNIQUE INDEX IF NOT EXISTS uq_users_username_lower ON users (LOWER(username));
CREATE UNIQUE INDEX IF NOT EXISTS uq_users_email_lower ON users (LOWER(email));
CREATE INDEX IF NOT EXISTS idx_tx_user_date ON transactions (user_id, post_date DESC, enter_date DESC);
CREATE INDEX IF NOT EXISTS idx_accounts_user_code ON accounts (user_id, code, name);
CREATE INDEX IF NOT EXISTS idx_splits_acct_rec ON splits (account_id, reconciled_state);

-- Token revocation support on password change / reset (Issue #8)
ALTER TABLE users ADD COLUMN IF NOT EXISTS token_version INTEGER NOT NULL DEFAULT 0;

-- Widen currency for non-currency commodities / ISINs (Issue #9)
ALTER TABLE accounts ALTER COLUMN currency TYPE VARCHAR(64);

-- Support commodities namespace/fraction and split quantities (Issue #9)
ALTER TABLE accounts ADD COLUMN IF NOT EXISTS commodity_namespace VARCHAR(32) DEFAULT 'CURRENCY';
ALTER TABLE accounts ADD COLUMN IF NOT EXISTS commodity_fraction BIGINT DEFAULT 100;
ALTER TABLE splits ADD COLUMN IF NOT EXISTS quantity NUMERIC(20, 6);
UPDATE splits SET quantity = amount WHERE quantity IS NULL;

-- Enforce normalized reconciled states (Issue #14, Issue #30)
UPDATE splits SET reconciled_state = LOWER(reconciled_state) WHERE reconciled_state <> LOWER(reconciled_state);
UPDATE splits SET reconciled_state = 'n', reconciled_date = NULL WHERE reconciled_state NOT IN ('n', 'c', 'y');

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_splits_reconciled_state') THEN
        ALTER TABLE splits ADD CONSTRAINT chk_splits_reconciled_state CHECK (reconciled_state IN ('n', 'c', 'y'));
    END IF;
END $$;
