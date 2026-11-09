ALTER TABLE lexemes ADD COLUMN ipa_uk TEXT NOT NULL DEFAULT '';
ALTER TABLE lexemes ADD COLUMN ipa_us TEXT NOT NULL DEFAULT '';
CREATE TABLE word_audio (profile_id TEXT NOT NULL REFERENCES profiles(id), lexeme_id TEXT NOT NULL REFERENCES lexemes(id), accent TEXT NOT NULL CHECK(accent IN ('uk','us')), blob_key TEXT NOT NULL, format TEXT NOT NULL CHECK(format IN ('mp3','wav')), bytes INTEGER NOT NULL CHECK(bytes>0), duration REAL, source TEXT NOT NULL, created_at INTEGER NOT NULL, PRIMARY KEY(profile_id,lexeme_id,accent));
CREATE INDEX word_audio_missing ON word_audio(profile_id,accent);
CREATE VIEW card_view AS SELECT c.*, l.ipa_uk AS ipa_uk, l.ipa_us AS ipa_us, EXISTS(SELECT 1 FROM word_audio w WHERE w.profile_id=c.profile_id AND w.lexeme_id=c.lexeme_id AND w.accent='uk') AS audio_uk, EXISTS(SELECT 1 FROM word_audio w WHERE w.profile_id=c.profile_id AND w.lexeme_id=c.lexeme_id AND w.accent='us') AS audio_us FROM cards c JOIN lexemes l ON l.id=c.lexeme_id;
