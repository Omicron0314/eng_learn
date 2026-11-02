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
    /// Position is a recoverable projection, so it has no recursively generated event.
    pub fn save_reading_position(
        &self,
        c: &ReaderCommand,
        id: &str,
        position: usize,
    ) -> Result<()> {
        let mut db = self.connection.lock().map_err(unavailable)?;
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        let text = body(&tx, c)?;
        if position > text.len() || !text.is_char_boundary(position) {
            return Err(invalid("阅读位置无效"));
        }
        let changed=tx.execute("UPDATE reading_sessions SET position=?1 WHERE profile_id=?2 AND id=?3 AND article_id=?4",params![i64::try_from(position).map_err(|_|invalid("位置超出范围"))?,c.profile,id,c.article]).map_err(unavailable)?;
        if changed != 1 {
            return Err(AppError::new(ErrorCode::NotFound, "阅读会话不存在"));
        }
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
