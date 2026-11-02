-- Recoverable device-local ordering metadata; deliberately excluded from backups/events.
CREATE TABLE reading_position_revisions (
    session_id TEXT PRIMARY KEY REFERENCES reading_sessions(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL CHECK(revision > 0)
);
