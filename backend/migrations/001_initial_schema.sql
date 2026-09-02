CREATE TABLE IF NOT EXISTS accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    code VARCHAR(50),
    account_type VARCHAR(50) NOT NULL, -- asset, bank, cash, credit, liability, equity, income, expense, receivable, payable
    parent_id UUID REFERENCES accounts(id) ON DELETE CASCADE,
    currency VARCHAR(10) NOT NULL DEFAULT 'INR',
    description TEXT,
    placeholder BOOLEAN NOT NULL DEFAULT FALSE,
    hidden BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    post_date DATE NOT NULL,
    enter_date TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    num VARCHAR(50),
    description VARCHAR(255) NOT NULL,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS splits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id UUID NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    amount NUMERIC(14, 2) NOT NULL, -- Positive = Debit, Negative = Credit
    memo TEXT,
    reconciled_state CHAR(1) NOT NULL DEFAULT 'n', -- 'n': not reconciled, 'c': cleared, 'y': reconciled
    reconciled_date TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_splits_account_id ON splits(account_id);
CREATE INDEX IF NOT EXISTS idx_splits_transaction_id ON splits(transaction_id);
CREATE INDEX IF NOT EXISTS idx_transactions_post_date ON transactions(post_date);
CREATE INDEX IF NOT EXISTS idx_accounts_parent_id ON accounts(parent_id);
