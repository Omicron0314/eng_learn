-- Historical reviews may not contain an after-state; do not invent one.
CREATE TABLE review_logs_v4 (id TEXT PRIMARY KEY, profile_id TEXT NOT NULL REFERENCES profiles(id), card_id TEXT NOT NULL, operation_id TEXT NOT NULL, rating TEXT NOT NULL CHECK(rating IN ('again','hard','good','easy')), state_before TEXT NOT NULL, state_after TEXT, due_before INTEGER NOT NULL, due_after INTEGER, scheduler_version TEXT NOT NULL, reviewed_at INTEGER NOT NULL, duration_ms INTEGER NOT NULL CHECK(duration_ms>=0), metadata TEXT NOT NULL DEFAULT '{}', FOREIGN KEY(profile_id,card_id) REFERENCES cards(profile_id,id), UNIQUE(profile_id,operation_id));
INSERT INTO review_logs_v4 SELECT * FROM review_logs;
DROP TABLE review_logs;
ALTER TABLE review_logs_v4 RENAME TO review_logs;
CREATE INDEX reviews_profile_time ON review_logs(profile_id,reviewed_at);
CREATE INDEX reviews_card_time ON review_logs(card_id,reviewed_at);
CREATE TRIGGER reviews_no_update BEFORE UPDATE ON review_logs BEGIN SELECT RAISE(ABORT,'review logs are append-only'); END;
CREATE TRIGGER reviews_no_delete BEFORE DELETE ON review_logs BEGIN SELECT RAISE(ABORT,'review logs are append-only'); END;
CREATE TABLE audio_reservations_v4 (id TEXT PRIMARY KEY, profile_id TEXT NOT NULL REFERENCES profiles(id), operation_id TEXT NOT NULL, article_id TEXT, chars INTEGER NOT NULL CHECK(chars>=0), reserved_at INTEGER NOT NULL, outcome TEXT NOT NULL, UNIQUE(profile_id,operation_id), FOREIGN KEY(profile_id,article_id) REFERENCES articles(profile_id,id));
INSERT INTO audio_reservations_v4 SELECT * FROM audio_reservations;
DROP TABLE audio_reservations;
ALTER TABLE audio_reservations_v4 RENAME TO audio_reservations;
CREATE INDEX reservations_profile_time ON audio_reservations(profile_id,reserved_at);
CREATE TABLE legacy_records (profile_id TEXT NOT NULL REFERENCES profiles(id),source_fingerprint TEXT NOT NULL,source_table TEXT NOT NULL,source_id TEXT NOT NULL,payload_json TEXT NOT NULL CHECK(json_valid(payload_json)),PRIMARY KEY(profile_id,source_fingerprint,source_table,source_id));
