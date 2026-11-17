//! GnuCash 5.17 SQLite schema, including the `versions` rows that stop GnuCash warning that the
//! file "is from an older version" (AGENTS.md section 5).

pub const GNUCASH_SCHEMA_SQL: &str = r#"
        CREATE TABLE gnclock (
            Hostname text(255),
            PID integer
        );

        CREATE TABLE versions (
            table_name text(50) PRIMARY KEY,
            table_version integer NOT NULL
        );

        CREATE TABLE books (
            guid text(32) PRIMARY KEY,
            root_account_guid text(32) NOT NULL,
            root_template_guid text(32) NOT NULL
        );

        CREATE TABLE commodities (
            guid text(32) PRIMARY KEY,
            namespace text(2048) NOT NULL,
            mnemonic text(2048) NOT NULL,
            fullname text(2048),
            cusip text(2048),
            fraction integer NOT NULL,
            quote_flag integer NOT NULL,
            quote_source text(2048)
        );

        CREATE TABLE accounts (
            guid text(32) PRIMARY KEY,
            name text(2048) NOT NULL,
            account_type text(2048) NOT NULL,
            commodity_guid text(32),
            commodity_scu integer NOT NULL,
            non_std_scu integer NOT NULL,
            parent_guid text(32),
            code text(2048),
            description text(2048),
            hidden integer,
            placeholder integer
        );

        CREATE TABLE transactions (
            guid text(32) PRIMARY KEY,
            currency_guid text(32) NOT NULL,
            num text(2048) NOT NULL,
            post_date text(14),
            enter_date text(14),
            description text(2048)
        );

        CREATE TABLE splits (
            guid text(32) PRIMARY KEY,
            tx_guid text(32) NOT NULL,
            account_guid text(32) NOT NULL,
            memo text(2048) NOT NULL,
            action text(2048) NOT NULL,
            reconcile_state text(1) NOT NULL,
            reconcile_date text(14),
            value_num integer NOT NULL,
            value_denom integer NOT NULL,
            quantity_num integer NOT NULL,
            quantity_denom integer NOT NULL,
            lot_guid text(32)
        );

        CREATE TABLE slots (
            id integer PRIMARY KEY AUTOINCREMENT,
            obj_guid text(32) NOT NULL,
            name text(4096) NOT NULL,
            slot_type integer NOT NULL,
            int64_val integer,
            string_val text(4096),
            double_val real,
            timespec_val text(14),
            guid_val text(32),
            numeric_val_num integer,
            numeric_val_denom integer,
            gdate_val text(8)
        );

        CREATE INDEX tx_post_date_index ON transactions (post_date);
        CREATE INDEX splits_tx_guid_index ON splits (tx_guid);
        CREATE INDEX splits_account_guid_index ON splits (account_guid);
        CREATE INDEX slots_guid_index ON slots (obj_guid);

        INSERT INTO versions (table_name, table_version) VALUES
            ('Gnucash', 5000017),
            ('Gnucash-Resave', 19920),
            ('accounts', 1),
            ('books', 1),
            ('commodities', 1),
            ('transactions', 4),
            ('splits', 5),
            ('slots', 4),
            ('prices', 3),
            ('recurrences', 2),
            ('lots', 2),
            ('billterms', 2),
            ('customers', 2),
            ('employees', 2),
            ('entries', 4),
            ('invoices', 4),
            ('jobs', 1),
            ('orders', 1),
            ('taxtables', 2),
            ('taxtable_entries', 3),
            ('vendors', 1),
            ('budgets', 1),
            ('budget_amounts', 1),
            ('schedxactions', 1);
        "#;
