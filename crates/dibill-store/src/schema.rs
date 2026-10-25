use rusqlite::Connection;

/// Schema migrations, applied in order. `PRAGMA user_version` stores how many
/// have run. Never edit a released migration: append a new one.
const MIGRATIONS: &[&str] = &[
    // 1: groups, members, transactions with items and shares.
    "
    CREATE TABLE groups (
        id            BLOB PRIMARY KEY NOT NULL,
        name          TEXT NOT NULL,
        emoji         TEXT,
        kind          TEXT NOT NULL CHECK (kind IN ('trip', 'home', 'couple', 'event', 'project', 'other')),
        base_currency TEXT NOT NULL,
        created_at    INTEGER NOT NULL,
        archived_at   INTEGER
    ) STRICT;

    CREATE TABLE members (
        id           BLOB PRIMARY KEY NOT NULL,
        group_id     BLOB NOT NULL REFERENCES groups (id) ON DELETE CASCADE,
        display_name TEXT NOT NULL,
        user_id      TEXT,
        status       TEXT NOT NULL CHECK (status IN ('active', 'inactive')),
        created_at   INTEGER NOT NULL
    ) STRICT;
    CREATE INDEX members_group ON members (group_id);

    CREATE TABLE transactions (
        id         BLOB PRIMARY KEY NOT NULL,
        group_id   BLOB NOT NULL REFERENCES groups (id) ON DELETE CASCADE,
        kind       TEXT NOT NULL CHECK (kind IN ('expense', 'income', 'transfer')),
        title      TEXT NOT NULL,
        payer_id   BLOB NOT NULL REFERENCES members (id),
        currency   TEXT NOT NULL,
        fx_rate    TEXT,
        date_days  INTEGER NOT NULL,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        deleted_at INTEGER
    ) STRICT;
    CREATE INDEX transactions_group_date ON transactions (group_id, date_days);

    CREATE TABLE items (
        transaction_id BLOB NOT NULL REFERENCES transactions (id) ON DELETE CASCADE,
        position       INTEGER NOT NULL,
        label          TEXT,
        total_minor    INTEGER NOT NULL,
        PRIMARY KEY (transaction_id, position)
    ) STRICT;

    CREATE TABLE item_shares (
        transaction_id BLOB NOT NULL,
        position       INTEGER NOT NULL,
        member_id      BLOB NOT NULL REFERENCES members (id),
        parts          INTEGER,
        amount_minor   INTEGER,
        PRIMARY KEY (transaction_id, position, member_id),
        FOREIGN KEY (transaction_id, position) REFERENCES items (transaction_id, position) ON DELETE CASCADE,
        CHECK ((parts IS NULL) <> (amount_minor IS NULL))
    ) STRICT;
    CREATE INDEX item_shares_member ON item_shares (member_id);
    ",
];

/// Brings the schema up to date in one transaction.
pub(crate) fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    let current: u32 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
    for (version, sql) in (1..).zip(MIGRATIONS).skip(current as usize) {
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", version)?;
    }
    tx.commit()
}

#[cfg(test)]
pub(crate) const LATEST: usize = MIGRATIONS.len();
