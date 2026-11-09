ALTER TABLE device_preferences ADD COLUMN pronunciation_accent TEXT NOT NULL DEFAULT 'us' CHECK(pronunciation_accent IN ('uk','us'));
ALTER TABLE device_preferences ADD COLUMN review_auto_play INTEGER NOT NULL DEFAULT 1 CHECK(review_auto_play IN (0,1));
