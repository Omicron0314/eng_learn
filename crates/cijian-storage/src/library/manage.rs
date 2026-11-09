use super::*;
use rusqlite::Connection;
#[derive(serde::Serialize)]
struct Affected {
    id: String,
    revision: i64,
    history: bool,
    shared: bool,
}
fn affected(db: &Connection, profile: &str, book: &str) -> Result<(DeletePreview, Vec<Affected>)> {
    let kind: Option<String> = db
        .query_row(
            "SELECT kind FROM word_books WHERE profile_id=?1 AND id=?2 AND archived_at IS NULL",
            params![profile, book],
            |r| r.get(0),
        )
        .optional()
        .map_err(unavailable)?;
    let kind = kind.ok_or(AppError::new(ErrorCode::NotFound, "词书不存在或已删除"))?;
    let entries: i64 = db
        .query_row(
            "SELECT count(*) FROM book_entries WHERE profile_id=?1 AND book_id=?2",
            params![profile, book],
            |r| r.get(0),
        )
        .map_err(unavailable)?;
    let mut cards = Vec::new();
    if kind == "study" {
        let mut statement=db.prepare("SELECT c.id,c.revision,EXISTS(SELECT 1 FROM review_logs r WHERE r.profile_id=c.profile_id AND r.card_id=c.id),EXISTS(SELECT 1 FROM book_entries e JOIN word_books b ON b.id=e.book_id WHERE e.profile_id=c.profile_id AND e.lexeme_id=c.lexeme_id AND b.id<>?2 AND b.kind='study' AND b.archived_at IS NULL) FROM cards c JOIN book_entries e ON e.lexeme_id=c.lexeme_id AND e.profile_id=c.profile_id WHERE c.profile_id=?1 AND e.book_id=?2 ORDER BY c.id").map_err(unavailable)?;
        cards = statement
            .query_map(params![profile, book], |r| {
                Ok(Affected {
                    id: r.get(0)?,
                    revision: r.get(1)?,
                    history: r.get(2)?,
                    shared: r.get(3)?,
                })
            })
            .map_err(unavailable)?
            .collect::<std::result::Result<_, _>>()
            .map_err(unavailable)?;
    }
    let fingerprint = super::super::hash(
        &json!({"book":book,"kind":kind,"entries":entries,"cards":cards}).to_string(),
    );
    Ok((
        DeletePreview {
            book_id: book.into(),
            removed_entries: entries as u64,
            shared_cards: cards.iter().filter(|c| c.shared).count() as u64,
            deleted_cards: cards.iter().filter(|c| !c.shared && !c.history).count() as u64,
            archived_cards: cards.iter().filter(|c| !c.shared && c.history).count() as u64,
            fingerprint,
        },
        cards,
    ))
}
impl Store {
    pub fn books(&self, profile: &str) -> Result<Vec<BookView>> {
        let db = self.connection.lock().map_err(unavailable)?;
        let mut statement=db.prepare("SELECT b.id,b.name,b.kind,b.revision,(SELECT count(*) FROM book_entries e WHERE e.book_id=b.id) FROM word_books b WHERE b.profile_id=?1 AND b.archived_at IS NULL ORDER BY b.created_at,b.id").map_err(unavailable)?;
        statement
            .query_map([profile], |r| {
                let kind: String = r.get(2)?;
                let revision: i64 = r.get(3)?;
                let entries: i64 = r.get(4)?;
                Ok(BookView {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    kind: if kind == "study" {
                        BookKind::Study
                    } else {
                        BookKind::Baseline
                    },
                    revision: revision as u64,
                    entries: entries as u64,
                })
            })
            .map_err(unavailable)?
            .collect::<std::result::Result<_, _>>()
            .map_err(unavailable)
    }
    pub fn edit_card(&self, c: EditCard) -> Result<CardView> {
        if c.meaning.chars().count() > 1000 || c.example.chars().count() > 5000 {
            return Err(invalid("释义或例句超出长度范围"));
        }
        let revision = i64::try_from(c.expected_revision).map_err(|_| invalid("版本无效"))?;
        let request=json!({"card":c.card_id,"revision":c.expected_revision,"meaning":c.meaning,"example":c.example,"suspended":c.suspended}).to_string();
        let result=self.command(&c.profile_id,&c.operation_id,"edit_card",&request,c.now,|tx|{
            let old=tx.query_row("SELECT * FROM card_view WHERE profile_id=?1 AND id=?2",params![c.profile_id,c.card_id],read_card).optional().map_err(unavailable)?.ok_or(AppError::new(ErrorCode::NotFound,"词卡不存在"))?;
            if old.archived&&!c.suspended{return Err(invalid("归档词卡不能通过暂停开关恢复"));}
            if old.revision!=c.expected_revision{return Err(AppError::new(ErrorCode::StaleState,"词卡已变更，请重新加载"));}
            let source=if c.meaning!=old.meaning{"user"}else{&old.text_source};
            let changed=tx.execute("UPDATE cards SET meaning=?1,example=?2,suspended=?3,text_source=?4,updated_at=?5,revision=revision+1 WHERE profile_id=?6 AND id=?7 AND revision=?8",params![c.meaning,c.example,c.suspended,source,c.now,c.profile_id,c.card_id,revision]).map_err(unavailable)?;
            if changed!=1{return Err(AppError::new(ErrorCode::StaleState,"词卡已变更"));}
            let value=tx.query_row("SELECT * FROM card_view WHERE profile_id=?1 AND id=?2",params![c.profile_id,c.card_id],read_card).map_err(unavailable)?;
            let mut fields = Vec::new();
            if c.meaning != old.meaning { fields.push("meaning"); }
            if c.example != old.example { fields.push("example"); }
            if c.suspended != old.suspended { fields.push("suspended"); }
            Ok((serde_json::to_value(value).map_err(unavailable)?,vec![Event{id:c.event_id,kind:"card_edit".into(),schema_version:1,payload:json!({"cardId":c.card_id,"changedFields":fields})}]))
        })?;
        serde_json::from_value(result).map_err(unavailable)
    }
    pub fn preview_delete_book(&self, profile: &str, book: &str) -> Result<DeletePreview> {
        let db = self.connection.lock().map_err(unavailable)?;
        affected(&db, profile, book).map(|(preview, _)| preview)
    }
    pub fn delete_book(&self, c: DeleteBook) -> Result<DeletePreview> {
        let request = json!({"book":c.book_id,"fingerprint":c.fingerprint}).to_string();
        let result=self.command(&c.profile_id,&c.operation_id,"delete_book",&request,c.now,|tx|{
            let (preview,cards)=affected(tx,&c.profile_id,&c.book_id)?;
            if preview.fingerprint!=c.fingerprint{return Err(AppError::new(ErrorCode::StaleState,"词书内容或复习历史已变化，请重新确认删除"));}
            for card in cards.iter().filter(|c|!c.shared){
                if card.history{tx.execute("UPDATE cards SET archived_at=?1,suspended=1,updated_at=?1,revision=revision+1 WHERE profile_id=?2 AND id=?3",params![c.now,c.profile_id,card.id]).map_err(unavailable)?;}
                else{tx.execute("DELETE FROM cards WHERE profile_id=?1 AND id=?2",params![c.profile_id,card.id]).map_err(unavailable)?;}
            }
            tx.execute("DELETE FROM book_entries WHERE profile_id=?1 AND book_id=?2",params![c.profile_id,c.book_id]).map_err(unavailable)?;
            tx.execute("UPDATE word_books SET archived_at=?1,updated_at=?1,revision=revision+1 WHERE profile_id=?2 AND id=?3",params![c.now,c.profile_id,c.book_id]).map_err(unavailable)?;
            Ok((serde_json::to_value(&preview).map_err(unavailable)?,vec![Event{id:c.event_id,kind:"book_delete".into(),schema_version:1,payload:serde_json::to_value(preview).map_err(unavailable)?}]))
        })?;
        serde_json::from_value(result).map_err(unavailable)
    }
}
