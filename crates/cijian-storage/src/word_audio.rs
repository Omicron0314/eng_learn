//! Word-level pronunciation storage. Audio is content-addressed by the caller and
//! kept separate from article audio so pack size, quota and cleanup stay independent.
use super::{Store, invalid, unavailable};
use cijian_ports::{
    Result,
    library::{SaveWordAudio, WordAudioAsset},
};
use rusqlite::{OptionalExtension, params};

impl Store {
    /// Lexemes that still miss at least one accent, in stable order, for resumable filling.
    pub fn words_missing_audio(&self, profile: &str) -> Result<Vec<(String, String)>> {
        let db = self.connection.lock().map_err(unavailable)?;
        let mut statement = db
            .prepare(
                "SELECT l.id,l.lemma FROM lexemes l JOIN cards c ON c.lexeme_id=l.id AND c.profile_id=?1 \
                 WHERE NOT EXISTS(SELECT 1 FROM word_audio w WHERE w.profile_id=?1 AND w.lexeme_id=l.id AND w.accent='uk') \
                    OR NOT EXISTS(SELECT 1 FROM word_audio w WHERE w.profile_id=?1 AND w.lexeme_id=l.id AND w.accent='us') \
                 GROUP BY l.id ORDER BY l.lemma,l.id",
            )
            .map_err(unavailable)?;
        let words = statement
            .query_map([profile], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(unavailable)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(unavailable)?;
        Ok(words)
    }
    pub fn word_audio(
        &self,
        profile: &str,
        lexeme: &str,
        accent: cijian_domain::audio::Accent,
    ) -> Result<Option<WordAudioAsset>> {
        let db = self.connection.lock().map_err(unavailable)?;
        let row: Option<(String, String, i64, String)> = db
            .query_row(
                "SELECT blob_key,format,bytes,accent FROM word_audio WHERE profile_id=?1 AND lexeme_id=?2 AND accent=?3",
                params![profile, lexeme, accent.key()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .map_err(unavailable)?;
        let Some((blob_key, format, bytes, stored)) = row else {
            return Ok(None);
        };
        let accent = cijian_domain::audio::Accent::parse(&stored)
            .ok_or_else(|| invalid("发音口音记录无效"))?;
        Ok(Some(WordAudioAsset {
            lexeme_id: lexeme.into(),
            accent,
            blob_key,
            format,
            bytes: u64::try_from(bytes).map_err(unavailable)?,
        }))
    }
    /// Records one accent recording plus any transcription learned with it.
    /// The caller writes the blob first; this write stays atomic and event-free because
    /// the recording is derived data that a resumed fill can rebuild.
    pub fn save_word_audio(&self, c: SaveWordAudio) -> Result<()> {
        if c.blob_key.is_empty()
            || !["mp3", "wav"].contains(&c.format.as_str())
            || c.bytes == 0
            || c.source.trim().is_empty()
            || c.duration.is_some_and(|d| !d.is_finite() || d <= 0.)
        {
            return Err(invalid("发音记录无效"));
        }
        let bytes = i64::try_from(c.bytes).map_err(|_| invalid("发音体积无效"))?;
        let duration = c.duration;
        let mut db = self.connection.lock().map_err(unavailable)?;
        let tx = db.transaction().map_err(unavailable)?;
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM lexemes WHERE id=?1)",
                [&c.lexeme_id],
                |r| r.get(0),
            )
            .map_err(unavailable)?;
        if !exists {
            return Err(invalid("词元不存在"));
        }
        tx.execute(
            "INSERT INTO word_audio(profile_id,lexeme_id,accent,blob_key,format,bytes,duration,source,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9) \
             ON CONFLICT(profile_id,lexeme_id,accent) DO UPDATE SET blob_key=excluded.blob_key,format=excluded.format,bytes=excluded.bytes,duration=excluded.duration,source=excluded.source,created_at=excluded.created_at",
            params![
                c.profile_id,
                c.lexeme_id,
                c.accent.key(),
                c.blob_key,
                c.format,
                bytes,
                duration,
                c.source,
                c.now
            ],
        )
        .map_err(unavailable)?;
        if let Some(ipa) = c.ipa_uk.filter(|v| !v.trim().is_empty()) {
            tx.execute(
                "UPDATE lexemes SET ipa_uk=?1 WHERE id=?2 AND ipa_uk=''",
                params![ipa.trim(), c.lexeme_id],
            )
            .map_err(unavailable)?;
        }
        if let Some(ipa) = c.ipa_us.filter(|v| !v.trim().is_empty()) {
            tx.execute(
                "UPDATE lexemes SET ipa_us=?1 WHERE id=?2 AND ipa_us=''",
                params![ipa.trim(), c.lexeme_id],
            )
            .map_err(unavailable)?;
        }
        tx.commit().map_err(unavailable)
    }
}
