-- Add durable non-secret request snapshots without changing the initial migration.
CREATE TABLE generation_snapshots (
    job_id TEXT PRIMARY KEY REFERENCES jobs(id),
    snapshot_json TEXT NOT NULL CHECK(json_valid(snapshot_json))
);
