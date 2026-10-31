-- Receipt scans (ADR 0008): one row per request to the model provider, for the per-device daily
-- limit, the monthly budget and cost tracking. No image or result is stored.

CREATE TABLE receipt_scans (
    id            BIGSERIAL PRIMARY KEY,
    device_id     UUID NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    model         TEXT NOT NULL,
    -- Filled when the provider answers: whether a receipt came back, tokens and time.
    ok            BOOLEAN,
    requests      SMALLINT,
    input_tokens  BIGINT,
    output_tokens BIGINT,
    elapsed_ms    INTEGER
);

CREATE INDEX receipt_scans_device_time ON receipt_scans (device_id, created_at);
CREATE INDEX receipt_scans_time ON receipt_scans (created_at);
