-- Devices, shared groups and the op relay (ADR 0006).

CREATE TABLE devices (
    id         UUID PRIMARY KEY,
    token_hash BYTEA NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE groups (
    id          UUID PRIMARY KEY,
    owner       UUID NOT NULL REFERENCES devices (id),
    -- Sequence number of the group's latest op.
    last_seq    BIGINT NOT NULL DEFAULT 0,
    invite_code TEXT UNIQUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE group_members (
    group_id  UUID NOT NULL REFERENCES groups (id) ON DELETE CASCADE,
    device_id UUID NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    joined_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (group_id, device_id)
);

CREATE TABLE ops (
    group_id    UUID NOT NULL REFERENCES groups (id) ON DELETE CASCADE,
    seq         BIGINT NOT NULL,
    id          UUID NOT NULL UNIQUE,
    lamport     BIGINT NOT NULL CHECK (lamport >= 0),
    device      UUID NOT NULL,
    entity      TEXT NOT NULL CHECK (entity IN ('group', 'member', 'transaction')),
    entity_id   UUID NOT NULL,
    field       TEXT,
    value       TEXT,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (group_id, seq),
    CHECK ((field IS NULL) = (value IS NULL))
);
