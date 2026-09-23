-- Posts the sleepless watcher has already read. A row here means the post
-- cannot open a second signal.

CREATE TABLE IF NOT EXISTS watched_posts (
    post_id VARCHAR(160) PRIMARY KEY,
    handle VARCHAR(64) NOT NULL,
    body TEXT NOT NULL,
    queued BOOLEAN NOT NULL DEFAULT FALSE,
    seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_watched_posts_seen ON watched_posts (seen_at DESC);
