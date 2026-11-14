-- 003_optimizations_and_constraints.sql
-- Performance indexes and case-insensitive uniqueness constraints

CREATE UNIQUE INDEX IF NOT EXISTS uq_users_username_lower ON users (LOWER(username));
CREATE UNIQUE INDEX IF NOT EXISTS uq_users_email_lower ON users (LOWER(email));
CREATE INDEX IF NOT EXISTS idx_tx_user_date ON transactions (user_id, post_date DESC, enter_date DESC);
CREATE INDEX IF NOT EXISTS idx_accounts_user_code ON accounts (user_id, code, name);
CREATE INDEX IF NOT EXISTS idx_splits_acct_rec ON splits (account_id, reconciled_state);
