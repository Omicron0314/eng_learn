#[cfg(test)]
mod tests;
use super::*;
use cijian_domain::{reading, settings::LookupMode, srs::SCHEDULER_VERSION};
use cijian_ports::inbox::*;
use serde_json::json;
use std::collections::BTreeSet;
struct AddedCard {
    id: String,
    created: bool,
    paused: bool,
    archived: bool,
}
fn add_card(
    tx: &Transaction<'_>,
    profile: &str,
    lexeme: &str,
    id: &str,
    meaning: &str,
    now: i64,
) -> Result<AddedCard> {
    let existing:Option<(String,bool,bool)>=tx.query_row("SELECT id,suspended,archived_at IS NOT NULL FROM cards WHERE profile_id=?1 AND lexeme_id=?2",params![profile,lexeme],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(unavailable)?;
    if let Some((id, paused, archived)) = existing {
        if !meaning.is_empty() {
            tx.execute(
                "UPDATE cards SET meaning=?1,text_source='lookup',updated_at=?2,revision=revision+1 WHERE profile_id=?3 AND id=?4 AND meaning='' AND text_source!='user'",
                params![meaning, now, profile, id],
            )
            .map_err(unavailable)?;
        }
        return Ok(AddedCard {
            id,
            paused,
            archived,
            created: false,
        });
    }
    let lemma: String = tx
        .query_row("SELECT lemma FROM lexemes WHERE id=?1", [lexeme], |r| {
            r.get(0)
        })
        .map_err(unavailable)?;
    tx.execute("INSERT INTO cards(id,profile_id,lexeme_id,front,meaning,example,text_source,origin,state,due,stability,difficulty,reps,lapses,elapsed_days,scheduled_days,learning_steps,ease_factor,scheduler_version,created_at,updated_at,revision) VALUES (?1,?2,?3,?4,?5,'',?6,'lookup','new',?7,0,0,0,0,0,0,0,2500,?8,?7,?7,1)",params![id,profile,lexeme,lemma,meaning,if meaning.is_empty(){"missing"}else{"lookup"},now,SCHEDULER_VERSION]).map_err(unavailable)?;
    Ok(AddedCard {
        id: id.into(),
        created: true,
        paused: false,
        archived: false,
    })
}
impl Store {
    pub fn lookup_mark(&self, profile: &str, lemma: &str) -> Result<(u64, String)> {
        let db = self.connection.lock().map_err(unavailable)?;
        let revision:i64=db.query_row("SELECT COALESCE(m.revision,0) FROM lexemes l LEFT JOIN lexeme_marks m ON m.lexeme_id=l.id AND m.profile_id=?1 WHERE l.lemma=?2 AND l.pos='' AND l.kind='word'",params![profile,lemma],|r|r.get(0)).optional().map_err(unavailable)?.unwrap_or(0);
        let meaning:String=db.query_row("SELECT c.meaning FROM cards c JOIN lexemes l ON l.id=c.lexeme_id WHERE c.profile_id=?1 AND l.lemma=?2 AND l.kind='word' AND l.pos=''",params![profile,lemma],|r|r.get(0)).optional().map_err(unavailable)?.unwrap_or_default();
        Ok((u64::try_from(revision).map_err(unavailable)?, meaning))
    }
    pub fn mark_word(&self, c: MarkWord) -> Result<MarkResult> {
        if c.meaning.chars().count() > 1000 {
            return Err(invalid("释义最多1000字符"));
        }
        let context = &c.context;
        let request=json!({"article":context.article,"hash":context.body_hash,"range":c.range,"action":c.action,"revision":c.expected_revision,"meaning":c.meaning}).to_string();
        let value=self.command(&context.profile,&context.operation,"mark_word",&request,context.now,|tx|{
            let body=super::reading::body(tx,context)?;let selection=reading::select(&body,c.range).map_err(|e|invalid(&e.to_string()))?;
            let lexeme:Option<String>=tx.query_row("SELECT id FROM lexemes WHERE lemma=?1 AND pos='' AND kind='word'",[&selection.lemma],|r|r.get(0)).optional().map_err(unavailable)?;
            let lexeme=match lexeme{Some(id)=>id,None=>{tx.execute("INSERT INTO lexemes VALUES (?1,?2,'','word')",params![c.lexeme_id,selection.lemma]).map_err(unavailable)?;c.lexeme_id.clone()}};
            let previous:i64=tx.query_row("SELECT revision FROM lexeme_marks WHERE profile_id=?1 AND lexeme_id=?2",params![context.profile,lexeme],|r|r.get(0)).optional().map_err(unavailable)?.unwrap_or(0);
            if previous<0 || previous as u64!=c.expected_revision{return Err(AppError::new(ErrorCode::StaleState,"词语标记已变更，请重新加载"));}
            let revision=if c.action==MarkAction::AddToReview{previous}else{previous.checked_add(1).ok_or(invalid("版本超出范围"))?};
            if c.action!=MarkAction::AddToReview{tx.execute("INSERT INTO lexeme_marks VALUES (?1,?2,?3,?4,?5) ON CONFLICT(profile_id,lexeme_id) DO UPDATE SET status=excluded.status,marked_at=excluded.marked_at,revision=excluded.revision",params![context.profile,lexeme,if c.action==MarkAction::Known{"known"}else{"unknown"},context.now,revision]).map_err(unavailable)?;}
            let settings:String=tx.query_row("SELECT json FROM settings WHERE profile_id=?1",[&context.profile],|r|r.get(0)).map_err(unavailable)?;
            let settings:Settings=serde_json::from_str(&settings).map_err(unavailable)?;
            let settings=ValidatedSettings::try_from(settings).map_err(unavailable)?;
            let mut result=MarkResult{lexeme_id:lexeme.clone(),revision:revision as u64,inbox_id:None,card_id:None,card_created:false,paused:false,archived:false};
            if c.action==MarkAction::AddToReview || (c.action==MarkAction::Unknown&&settings.get().lookup_mode==LookupMode::AutoAdd){
                let card=add_card(tx,&context.profile,&lexeme,&c.card_id,&c.meaning,context.now)?;
                result.card_id=Some(card.id);result.card_created=card.created;result.paused=card.paused;result.archived=card.archived;
                tx.execute("UPDATE inbox SET status='added',updated_at=?3,revision=revision+1 WHERE profile_id=?1 AND lexeme_id=?2 AND status='pending'",params![context.profile,lexeme,context.now]).map_err(unavailable)?;
            }else if c.action==MarkAction::Unknown{
                let existing:Option<String>=tx.query_row("SELECT id FROM inbox WHERE profile_id=?1 AND lexeme_id=?2 AND status='pending'",params![context.profile,lexeme],|r|r.get(0)).optional().map_err(unavailable)?;
                let id=existing.unwrap_or(c.inbox_id.clone());
                tx.execute("INSERT INTO inbox VALUES (?1,?2,?3,?4,?5,?6,?7,'pending',?8,?8,1) ON CONFLICT(profile_id,lexeme_id) WHERE status='pending' DO UPDATE SET article_id=excluded.article_id,body_hash=excluded.body_hash,context_sentence=excluded.context_sentence,range_json=excluded.range_json,updated_at=excluded.updated_at,revision=inbox.revision+1",params![id,context.profile,lexeme,context.article,context.body_hash,selection.sentence,serde_json::to_string(&c.range).map_err(unavailable)?,context.now]).map_err(unavailable)?;
                result.inbox_id=Some(id);
            }
            let kind=match c.action{MarkAction::Known=>"mark_known",MarkAction::Unknown=>"mark_unknown",MarkAction::AddToReview=>"add_to_review"};
            Ok((serde_json::to_value(&result).map_err(unavailable)?,vec![Event{id:context.event.clone(),kind:kind.into(),schema_version:1,payload:json!({"articleId":context.article,"lexemeId":lexeme,"inboxId":result.inbox_id,"cardId":result.card_id,"created":result.card_created,"paused":result.paused,"archived":result.archived,"bodyHash":context.body_hash,"range":c.range})}]))
        })?;
        serde_json::from_value(value).map_err(unavailable)
    }
    pub fn pending_inbox_count(&self, profile: &str) -> Result<u64> {
        self.connection
            .lock()
            .map_err(unavailable)?
            .query_row(
                "SELECT count(*) FROM inbox WHERE profile_id=?1 AND status='pending'",
                [profile],
                |r| r.get::<_, i64>(0),
            )
            .map_err(unavailable)
            .and_then(|count| u64::try_from(count).map_err(unavailable))
    }
    pub fn inbox(&self, profile: &str, page: u32) -> Result<Vec<InboxItem>> {
        let db = self.connection.lock().map_err(unavailable)?;
        let mut statement=db.prepare("SELECT i.id,l.lemma,i.article_id,a.title,i.context_sentence,i.body_hash,i.range_json,i.revision,COALESCE(c.meaning,''),COALESCE(c.text_source,'') FROM inbox i JOIN lexemes l ON l.id=i.lexeme_id LEFT JOIN articles a ON a.id=i.article_id AND a.profile_id=i.profile_id LEFT JOIN cards c ON c.lexeme_id=i.lexeme_id AND c.profile_id=i.profile_id WHERE i.profile_id=?1 AND i.status='pending' ORDER BY i.created_at DESC,i.id LIMIT 50 OFFSET ?2").map_err(unavailable)?;
        let rows = statement
            .query_map(params![profile, i64::from(page) * 50], |r| {
                let range: Option<String> = r.get(6)?;
                Ok((
                    InboxItem {
                        id: r.get(0)?,
                        lemma: r.get(1)?,
                        article_id: r.get(2)?,
                        article_title: r.get(3)?,
                        context: r.get(4)?,
                        body_hash: r.get(5)?,
                        range: None,
                        revision: u64::try_from(r.get::<_, i64>(7)?)
                            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(7, -1))?,
                        meaning: r.get(8)?,
                        meaning_source: r.get(9)?,
                    },
                    range,
                ))
            })
            .map_err(unavailable)?;
        rows.map(|row| {
            let (mut item, range) = row.map_err(unavailable)?;
            item.range = range
                .map(|r| serde_json::from_str(&r).map_err(unavailable))
                .transpose()?;
            Ok(item)
        })
        .collect()
    }
    pub fn batch_inbox(&self, c: BatchInbox) -> Result<BatchResult> {
        let mut ids = BTreeSet::new();
        if c.selections.is_empty()
            || c.selections.len() > 500
            || c.selections.iter().any(|s| !ids.insert(&s.id))
        {
            return Err(invalid("批处理须包含1–500个不重复条目"));
        }
        let request = json!({"action":c.action,"selections":c.selections}).to_string();
        let value=self.command(&c.profile,&c.operation,"batch_inbox",&request,c.now,|tx|{
            let mut report=BatchResult{processed:0,cards_created:0,paused:0,archived:0};let mut events=Vec::new();
            for selected in &c.selections{
                let lexeme:Option<String>=tx.query_row("SELECT lexeme_id FROM inbox WHERE profile_id=?1 AND id=?2 AND status='pending' AND revision=?3",params![c.profile,selected.id,i64::try_from(selected.revision).map_err(|_|invalid("版本超出范围"))?],|r|r.get(0)).optional().map_err(unavailable)?;
                let lexeme=lexeme.ok_or(AppError::new(ErrorCode::StaleState,"选中的生词已变化，请刷新后重试"))?;
                let mut card_id=None;let mut created=false;
                if c.action==InboxAction::Add{let card=add_card(tx,&c.profile,&lexeme,&selected.card_id,"",c.now)?;report.cards_created+=u64::from(card.created);report.paused+=u64::from(card.paused);report.archived+=u64::from(card.archived);created=card.created;card_id=Some(card.id);}
                let status=if c.action==InboxAction::Add{"added"}else{"ignored"};
                tx.execute("UPDATE inbox SET status=?1,updated_at=?2,revision=revision+1 WHERE profile_id=?3 AND id=?4",params![status,c.now,c.profile,selected.id]).map_err(unavailable)?;
                report.processed+=1;events.push(Event{id:selected.event_id.clone(),kind:if c.action==InboxAction::Add{"add_to_review"}else{"inbox_ignore"}.into(),schema_version:1,payload:json!({"inboxId":selected.id,"lexemeId":lexeme,"cardId":card_id,"created":created,"source":"inbox"})});
            }
            Ok((serde_json::to_value(report).map_err(unavailable)?,events))
        })?;
        serde_json::from_value(value).map_err(unavailable)
    }
}
