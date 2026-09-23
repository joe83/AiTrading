-- Lessons learned from closed trades, and the approved rules the decision prompt may use.
-- A lesson stays a hypothesis until it is approved into the playbook.

ALTER TABLE analysis_logs ALTER COLUMN grok_model TYPE VARCHAR(64);

CREATE INDEX IF NOT EXISTS idx_trades_open_signal
    ON trades (signal_id)
    WHERE closed_at IS NULL;

CREATE TABLE IF NOT EXISTS lessons (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    scope VARCHAR(64) NOT NULL,
    claim TEXT NOT NULL,
    evidence_trade_ids UUID[] NOT NULL DEFAULT '{}',
    sample_size INTEGER NOT NULL DEFAULT 0,
    status VARCHAR(16) NOT NULL DEFAULT 'hypothesis'
        CHECK (status IN ('hypothesis', 'supported', 'rejected')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_lessons_scope_status ON lessons (scope, status);

CREATE TABLE IF NOT EXISTS playbook (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    lesson_id UUID REFERENCES lessons(id) ON DELETE SET NULL,
    scope VARCHAR(64) NOT NULL,
    rule TEXT NOT NULL,
    sample_size INTEGER NOT NULL DEFAULT 0,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_playbook_active ON playbook (active, scope);
