#[cfg(test)]
mod tests;
use super::*;
use cijian_domain::reading::{self, FeedbackMode, TextRange};
use cijian_ports::reading::*;
use serde_json::json;
pub(super) fn body(db: &Connection, c: &ReaderCommand) -> Result<String> {
    let value: Option<(String, String)> = db
        .query_row(
            "SELECT body,body_hash FROM articles WHERE profile_id=?1 AND id=?2",
            params![c.profile, c.article],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(unavailable)?;
    let (text, hash) = value.ok_or(AppError::new(ErrorCode::NotFound, "文章不存在"))?;
    if hash != c.body_hash {
        return Err(AppError::new(
            ErrorCode::StaleState,
            "正文版本已变化，请重新打开文章",
        ));
    }
    Ok(text)
}
fn session(db: &Connection, profile: &str, id: &str) -> Result<ReadingSession> {
    db.query_row("SELECT id,article_id,position,opened_at,completed_at FROM reading_sessions WHERE profile_id=?1 AND id=?2",params![profile,id],|r|Ok(ReadingSession{id:r.get(0)?,article_id:r.get(1)?,position:usize::try_from(r.get::<_,i64>(2)?).map_err(|_|rusqlite::Error::IntegralValueOutOfRange(2,-1))?,opened_at:r.get(3)?,completed_at:r.get(4)?})).optional().map_err(unavailable)?.ok_or(AppError::new(ErrorCode::NotFound,"阅读会话不存在"))
}
impl Store {
    pub fn cached_gloss(
        &self,
        profile: &str,
        lemma: &str,
        context_hash: &str,
        fingerprint: &str,
    ) -> Result<Option<cijian_domain::gloss::Gloss>> {
        let db = self.connection.lock().map_err(unavailable)?;
        let value:Option<String>=db.query_row("SELECT g.gloss_json FROM gloss_cache g JOIN lexemes l ON l.id=g.lexeme_id WHERE g.profile_id=?1 AND l.lemma=?2 AND l.pos='' AND l.kind='word' AND g.context_hash=?3 AND g.prompt_version='gloss.v1' AND g.provider_fingerprint=?4",params![profile,lemma,context_hash,fingerprint],|r|r.get(0)).optional().map_err(unavailable)?;
        value
            .map(|v| cijian_domain::gloss::Gloss::parse(&v, lemma).map_err(unavailable))
            .transpose()
    }
    pub fn cache_gloss(
        &self,
        c: &ReaderCommand,
        range: TextRange,
        fingerprint: &str,
        gloss: &cijian_domain::gloss::Gloss,
        lexeme_id: &str,
    ) -> Result<()> {
        let request=json!({"article":c.article,"hash":c.body_hash,"range":range,"fingerprint":fingerprint,"gloss":gloss}).to_string();
        self.command(&c.profile,&c.operation,"cache_gloss",&request,c.now,|tx|{
            let text=body(tx,c)?;let selection=reading::select(&text,range).map_err(|e|invalid(&e.to_string()))?;
            let serialized=serde_json::to_string(gloss).map_err(unavailable)?;
            cijian_domain::gloss::Gloss::parse(&serialized,&selection.lemma).map_err(|e|invalid(&e.to_string()))?;
            tx.execute("INSERT INTO lexemes(id,lemma,pos,kind) VALUES (?1,?2,'','word') ON CONFLICT(lemma,pos,kind) DO NOTHING",params![lexeme_id,selection.lemma]).map_err(unavailable)?;
            let lexeme:String=tx.query_row("SELECT id FROM lexemes WHERE lemma=?1 AND pos='' AND kind='word'",[&selection.lemma],|r|r.get(0)).map_err(unavailable)?;
            tx.execute("INSERT INTO gloss_cache VALUES (?1,?2,?3,'gloss.v1',?4,?5,?6) ON CONFLICT(profile_id,lexeme_id,context_hash,prompt_version,provider_fingerprint) DO UPDATE SET gloss_json=excluded.gloss_json,created_at=excluded.created_at",params![c.profile,lexeme,hash(&selection.sentence),fingerprint,serialized,c.now]).map_err(unavailable)?;
            Ok((json!(true),vec![Event{id:c.event.clone(),kind:"lookup".into(),schema_version:1,payload:json!({"articleId":c.article,"lexemeId":lexeme,"bodyHash":c.body_hash,"range":range,"cacheHit":false,"status":"success"})}]))
        })?;
        Ok(())
    }
    pub fn lookup_context(&self, c: &ReaderCommand, range: TextRange) -> Result<LookupContext> {
        let db = self.connection.lock().map_err(unavailable)?;
        let text = body(&db, c)?;
        let selection = reading::select(&text, range).map_err(|e| invalid(&e.to_string()))?;
        Ok(LookupContext {
            article_id: c.article.clone(),
            body_hash: c.body_hash.clone(),
            selection,
        })
    }
    pub fn open_reading(&self, c: ReaderCommand, session_id: &str) -> Result<ReadingSession> {
        let request =
            json!({"article":c.article,"hash":c.body_hash,"session":session_id}).to_string();
        let result=self.command(&c.profile,&c.operation,"open_reading",&request,c.now,|tx|{
            body(tx,&c)?;
            let position:i64=tx.query_row("SELECT position FROM reading_sessions WHERE profile_id=?1 AND article_id=?2 ORDER BY opened_at DESC,rowid DESC LIMIT 1",params![c.profile,c.article],|r|r.get(0)).optional().map_err(unavailable)?.unwrap_or(0);
            tx.execute("INSERT INTO reading_sessions VALUES (?1,?2,?3,?4,NULL,?5)",params![session_id,c.profile,c.article,c.now,position]).map_err(unavailable)?;
            Ok((serde_json::to_value(session(tx,&c.profile,session_id)?).map_err(unavailable)?,vec![Event{id:c.event.clone(),kind:"article_open".into(),schema_version:1,payload:json!({"articleId":c.article,"sessionId":session_id,"via":"reading"})}]))
        })?;
        serde_json::from_value(result).map_err(unavailable)
    }
    /// Position and ordering are recoverable projections, without recursively generated events.
    /// Each new reading session starts at revision zero; late writes cannot move it backwards.
    pub fn save_reading_position(
        &self,
        c: &ReaderCommand,
        id: &str,
        position: usize,
        revision: u64,
    ) -> Result<()> {
        let revision = i64::try_from(revision)
            .ok()
            .filter(|revision| *revision > 0)
            .ok_or(invalid("阅读位置版本无效"))?;
        let mut db = self.connection.lock().map_err(unavailable)?;
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        let text = body(&tx, c)?;
        if position > text.len() || !text.is_char_boundary(position) {
            return Err(invalid("阅读位置无效"));
        }
        let previous:Option<(i64,i64)>=tx.query_row("SELECT s.position,COALESCE(r.revision,0) FROM reading_sessions s LEFT JOIN reading_position_revisions r ON r.session_id=s.id WHERE s.profile_id=?1 AND s.id=?2 AND s.article_id=?3",params![c.profile,id,c.article],|row|Ok((row.get(0)?,row.get(1)?))).optional().map_err(unavailable)?;
        let Some((previous_position, previous_revision)) = previous else {
            return Err(AppError::new(ErrorCode::NotFound, "阅读会话不存在"));
        };
        if revision < previous_revision
            || (revision == previous_revision
                && usize::try_from(previous_position).ok() != Some(position))
        {
            return Err(AppError::new(ErrorCode::StaleState, "阅读位置已更新"));
        }
        let changed=tx.execute("UPDATE reading_sessions SET position=?1 WHERE profile_id=?2 AND id=?3 AND article_id=?4",params![i64::try_from(position).map_err(|_|invalid("位置超出范围"))?,c.profile,id,c.article]).map_err(unavailable)?;
        if changed != 1 {
            return Err(AppError::new(ErrorCode::NotFound, "阅读会话不存在"));
        }
        tx.execute("INSERT INTO reading_position_revisions(session_id,revision) VALUES(?1,?2) ON CONFLICT(session_id) DO UPDATE SET revision=excluded.revision",params![id,revision]).map_err(unavailable)?;
        tx.commit().map_err(unavailable)
    }
    pub fn complete_reading(&self, c: ReaderCommand, id: &str) -> Result<ReadingSession> {
        let request = json!({"article":c.article,"hash":c.body_hash,"session":id}).to_string();
        let value=self.command(&c.profile,&c.operation,"complete_reading",&request,c.now,|tx|{
            body(tx,&c)?;let previous=session(tx,&c.profile,id)?;if previous.article_id!=c.article{return Err(invalid("会话不属于该文章"));}
            let changed=tx.execute("UPDATE reading_sessions SET completed_at=?1 WHERE profile_id=?2 AND id=?3 AND completed_at IS NULL",params![c.now,c.profile,id]).map_err(unavailable)?;
            Ok((serde_json::to_value(session(tx,&c.profile,id)?).map_err(unavailable)?,vec![Event{id:c.event.clone(),kind:if changed==1{"article_complete"}else{"article_complete_replayed"}.into(),schema_version:1,payload:json!({"articleId":c.article,"sessionId":id,"via":"reading"})}]))
        })?;
        serde_json::from_value(value).map_err(unavailable)
    }
    pub fn submit_feedback(&self, c: ReaderCommand, feedback: Feedback) -> Result<Feedback> {
        let request =
            json!({"article":c.article,"hash":c.body_hash,"feedback":feedback}).to_string();
        let value=self.command(&c.profile,&c.operation,"difficulty_feedback",&request,c.now,|tx|{
            body(tx,&c)?;
            Ok((serde_json::to_value(&feedback).map_err(unavailable)?,vec![Event{id:c.event.clone(),kind:"difficulty_feedback".into(),schema_version:1,payload:json!({"articleId":c.article,"mode":feedback.mode,"value":feedback.value})}]))
        })?;
        serde_json::from_value(value).map_err(unavailable)
    }
    pub fn feedback(
        &self,
        profile: &str,
        article: &str,
        mode: FeedbackMode,
    ) -> Result<Option<Feedback>> {
        let db = self.connection.lock().map_err(unavailable)?;
        let mode = match mode {
            FeedbackMode::Reading => "reading",
            FeedbackMode::Listening => "listening",
        };
        let payload:Option<String>=db.query_row("SELECT payload_json FROM events WHERE profile_id=?1 AND type='difficulty_feedback' AND json_extract(payload_json,'$.articleId')=?2 AND json_extract(payload_json,'$.mode')=?3 ORDER BY rowid DESC LIMIT 1",params![profile,article,mode],|r|r.get(0)).optional().map_err(unavailable)?;
        payload
            .map(|p| serde_json::from_str(&p).map_err(unavailable))
            .transpose()
    }
    pub fn flag_sentences(&self, c: ReaderCommand, flag: SentenceFlag) -> Result<()> {
        let request = json!({"article":c.article,"hash":c.body_hash,"flag":flag}).to_string();
        self.command(&c.profile,&c.operation,"flag_sentences",&request,c.now,|tx|{
            let text=body(tx,&c)?;reading::validate_flag(&text,&flag.ranges,&flag.note).map_err(|e|invalid(&e.to_string()))?;
            Ok((json!(true),vec![Event{id:c.event.clone(),kind:"unnatural_flag".into(),schema_version:1,payload:json!({"articleId":c.article,"bodyHash":c.body_hash,"ranges":flag.ranges,"note":flag.note})}]))
        })?;
        Ok(())
    }
}
/// On-demand review gloss. Unlike article lookup, the card itself supplies the
/// context, so phrases and cards without an example sentence remain supported.
#[derive(Debug, Clone)]
pub struct CardGloss {
    pub profile_id: String,
    pub operation_id: String,
    pub event_id: String,
    pub card_id: String,
    pub expected_revision: u64,
    pub sentence: String,
    pub fingerprint: String,
    pub gloss: cijian_domain::gloss::Gloss,
    pub now: i64,
}
impl Store {
    pub fn card_gloss_target(
        &self,
        profile: &str,
        card: &str,
    ) -> Result<(cijian_ports::library::CardView, String, String)> {
        let db = self.connection.lock().map_err(unavailable)?;
        let card = db
            .query_row(
                "SELECT * FROM card_view WHERE profile_id=?1 AND id=?2",
                params![profile, card],
                super::library::read_card,
            )
            .optional()
            .map_err(unavailable)?
            .ok_or(AppError::new(ErrorCode::NotFound, "词卡不存在"))?;
        let lemma: String = db
            .query_row(
                "SELECT lemma FROM lexemes WHERE id=?1",
                [&card.lexeme_id],
                |r| r.get(0),
            )
            .map_err(unavailable)?;
        let sentence = if card.example.trim().is_empty() {
            card.front.clone()
        } else {
            card.example.clone()
        };
        Ok((card, lemma, sentence))
    }
    pub fn card_gloss_cache(
        &self,
        profile: &str,
        lexeme: &str,
        sentence: &str,
        fingerprint: &str,
    ) -> Result<Option<cijian_domain::gloss::Gloss>> {
        let db = self.connection.lock().map_err(unavailable)?;
        let value: Option<String> = db
            .query_row(
                "SELECT gloss_json FROM gloss_cache WHERE profile_id=?1 AND lexeme_id=?2 AND context_hash=?3 AND prompt_version='gloss.v1' AND provider_fingerprint=?4",
                params![profile, lexeme, hash(sentence), fingerprint],
                |r| r.get(0),
            )
            .optional()
            .map_err(unavailable)?;
        value
            .map(|value| cijian_domain::gloss::Gloss::parse_any(&value))
            .transpose()
            .map_err(unavailable)
    }
    pub fn apply_card_gloss(&self, c: CardGloss) -> Result<cijian_ports::library::CardView> {
        let expected = i64::try_from(c.expected_revision).map_err(|_| invalid("版本无效"))?;
        let meaning = format!(
            "{} {}\n{}\n{}",
            c.gloss.pos, c.gloss.ipa, c.gloss.meaning_zh, c.gloss.meaning_en
        );
        if meaning.chars().count() > 1000 {
            return Err(invalid("释义超出长度范围"));
        }
        let request = json!({
            "card": c.card_id,
            "revision": c.expected_revision,
            "sentenceHash": hash(&c.sentence),
            "fingerprint": c.fingerprint,
            "gloss": c.gloss,
        })
        .to_string();
        let result = self.command(
            &c.profile_id,
            &c.operation_id,
            "card_gloss",
            &request,
            c.now,
            |tx| {
                let old = tx
                    .query_row(
                        "SELECT * FROM card_view WHERE profile_id=?1 AND id=?2",
                        params![c.profile_id, c.card_id],
                        super::library::read_card,
                    )
                    .optional()
                    .map_err(unavailable)?
                    .ok_or(AppError::new(ErrorCode::NotFound, "词卡不存在"))?;
                if old.revision != c.expected_revision {
                    return Err(AppError::new(
                        ErrorCode::StaleState,
                        "词卡已变更，请重新加载",
                    ));
                }
                let serialized = serde_json::to_string(&c.gloss).map_err(unavailable)?;
                cijian_domain::gloss::Gloss::parse_any(&serialized)
                    .map_err(|_| invalid("释义结构无效"))?;
                tx.execute(
                    "INSERT INTO gloss_cache VALUES (?1,?2,?3,'gloss.v1',?4,?5,?6) ON CONFLICT(profile_id,lexeme_id,context_hash,prompt_version,provider_fingerprint) DO UPDATE SET gloss_json=excluded.gloss_json,created_at=excluded.created_at",
                    params![c.profile_id, old.lexeme_id, hash(&c.sentence), c.fingerprint, serialized, c.now],
                )
                .map_err(unavailable)?;
                let changed = tx
                    .execute(
                        "UPDATE cards SET meaning=?1,text_source='generated',updated_at=?2,revision=revision+1 WHERE profile_id=?3 AND id=?4 AND revision=?5",
                        params![meaning, c.now, c.profile_id, c.card_id, expected],
                    )
                    .map_err(unavailable)?;
                if changed != 1 {
                    return Err(AppError::new(ErrorCode::StaleState, "词卡已变更"));
                }
                let value = tx
                    .query_row(
                        "SELECT * FROM card_view WHERE profile_id=?1 AND id=?2",
                        params![c.profile_id, c.card_id],
                        super::library::read_card,
                    )
                    .map_err(unavailable)?;
                Ok((
                    serde_json::to_value(value).map_err(unavailable)?,
                    vec![Event {
                        id: c.event_id,
                        kind: "card_gloss".into(),
                        schema_version: 1,
                        payload: json!({"cardId": c.card_id, "lexemeId": old.lexeme_id, "promptVersion": "gloss.v1"}),
                    }],
                ))
            },
        )?;
        serde_json::from_value(result).map_err(unavailable)
    }
}
