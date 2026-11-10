-- Receipt photos attached to expenses (ADR 0010). Opaque to the server: stored and served back
-- to the group's members only, removed with the group.

CREATE TABLE attachments (
    group_id   UUID NOT NULL REFERENCES groups (id) ON DELETE CASCADE,
    id         UUID NOT NULL,
    mime       TEXT NOT NULL,
    bytes      BYTEA NOT NULL,
    created_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (group_id, id)
);
