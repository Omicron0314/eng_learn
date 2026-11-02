mod manage;
#[cfg(test)]
mod tests;
use super::{Store, invalid, unavailable};
use cijian_domain::{
    import::BookKind,
    settings::{Settings, ValidatedSettings},
    srs::{self, SCHEDULER_VERSION, SrsState, State},
    time::study_day,
};
use cijian_ports::{AppError, ErrorCode, Event, Result, library::*};
use rusqlite::{OptionalExtension, Row, params};
use serde_json::json;

fn state_name(state: State) -> &'static str {
    match state {
        State::New => "new",
        State::Learning => "learning",
        State::Review => "review",
        State::Relearning => "relearning",
    }
}
fn read_card(row: &Row<'_>) -> rusqlite::Result<CardView> {
    let state: String = row.get("state")?;
    let state = match state.as_str() {
        "new" => State::New,
        "learning" => State::Learning,
        "review" => State::Review,
        "relearning" => State::Relearning,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    let revision: i64 = row.get("revision")?;
    Ok(CardView {
        id: row.get("id")?,
        lexeme_id: row.get("lexeme_id")?,
        front: row.get("front")?,
        meaning: row.get("meaning")?,
        example: row.get("example")?,
        text_source: row.get("text_source")?,
        suspended: row.get("suspended")?,
        archived: row.get::<_, Option<i64>>("archived_at")?.is_some(),
        revision: u64::try_from(revision).map_err(|_| rusqlite::Error::InvalidQuery)?,
        created_at: row.get("created_at")?,
        srs: SrsState {
            state,
            due: row.get("due")?,
            stability: row.get("stability")?,
            difficulty: row.get("difficulty")?,
            reps: row.get("reps")?,
            lapses: row.get("lapses")?,
            last_review: row.get("last_review")?,
            elapsed_days: row.get("elapsed_days")?,
            scheduled_days: row.get("scheduled_days")?,
            learning_steps: row.get("learning_steps")?,
        },
    })
}

impl Store {
    pub fn import(&self, c: ImportWords) -> Result<ImportReport> {
        if c.name.trim().is_empty()
            || c.row_ids.len() != c.parsed.rows.len()
            || c.source_hash.is_empty()
        {
            return Err(invalid("导入目标或行标识无效"));
        }
        for row in &c.parsed.rows {
            if row.word.trim().is_empty()
                || row.front.trim().is_empty()
                || row.meaning.chars().count() > 1000
                || row.example.chars().count() > 5000
            {
                return Err(invalid("导入词条无效"));
            }
        }
        let kind = if c.kind == BookKind::Baseline {
            "baseline"
        } else {
            "study"
        };
        let request=json!({"name":c.name,"kind":kind,"sourceHash":c.source_hash,"format":c.format,"rows":c.parsed}).to_string();
        let result=self.command(&c.profile_id,&c.operation_id,"import_words",&request,c.now,|tx|{
            let same:Option<String>=tx.query_row("SELECT id FROM word_books WHERE profile_id=?1 AND name=?2 AND kind=?3 AND archived_at IS NULL",params![c.profile_id,c.name,kind],|r|r.get(0)).optional().map_err(unavailable)?;
            let book=same.unwrap_or_else(||c.book_id.clone());
            tx.execute("INSERT OR IGNORE INTO word_books(id,profile_id,name,kind,source,created_at,updated_at,revision) VALUES (?1,?2,?3,?4,?5,?6,?6,1)",params![book,c.profile_id,c.name,kind,c.format,c.now]).map_err(unavailable)?;
            let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM word_books WHERE id=?1 AND profile_id=?2 AND kind=?3 AND archived_at IS NULL)",params![book,c.profile_id,kind],|r|r.get(0)).map_err(unavailable)?;
            if !valid{return Err(invalid("目标词书用途不匹配"));}
            tx.execute("INSERT INTO imports VALUES (?1,?2,?3,?4,?5,'{}',?6,?7)",params![c.import_id,c.profile_id,c.source_name,c.format,kind,c.source_hash,c.now]).map_err(unavailable)?;
            let mut report=ImportReport{parsed:c.parsed.parsed,duplicates:c.parsed.duplicates,phrases:c.parsed.phrases,skipped:c.parsed.skipped,errors:c.parsed.errors.clone(),..Default::default()};
            for (row,ids) in c.parsed.rows.iter().zip(&c.row_ids){
                let lexeme_kind=if row.phrase{"phrase"}else{"word"};
                let existing:Option<String>=tx.query_row("SELECT id FROM lexemes WHERE lemma=?1 AND pos='' AND kind=?2",params![row.word,lexeme_kind],|r|r.get(0)).optional().map_err(unavailable)?;
                let lexeme=match existing{Some(id)=>id,None=>{tx.execute("INSERT INTO lexemes VALUES (?1,?2,'',?3)",params![ids.lexeme,row.word,lexeme_kind]).map_err(unavailable)?;report.words_added+=1;ids.lexeme.clone()}};
                let added=tx.execute("INSERT INTO book_entries VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(book_id,lexeme_id) DO NOTHING",params![ids.entry,c.profile_id,book,lexeme,row.raw,c.import_id]).map_err(unavailable)?;
                report.entries_added+=added as u64;if added==0{report.duplicates+=1;}
                if c.kind==BookKind::Study {
                    let card_exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM cards WHERE profile_id=?1 AND lexeme_id=?2)",params![c.profile_id,lexeme],|r|r.get(0)).map_err(unavailable)?;
                    if !card_exists {
                        tx.execute("INSERT INTO cards(id,profile_id,lexeme_id,front,meaning,example,text_source,origin,state,due,stability,difficulty,reps,lapses,elapsed_days,scheduled_days,learning_steps,scheduler_version,created_at,updated_at,revision) VALUES (?1,?2,?3,?4,?5,?6,?7,'import','new',?8,0,0,0,0,0,0,0,?9,?8,?8,1)",params![ids.card,c.profile_id,lexeme,row.front,row.meaning,row.example,if row.meaning.is_empty(){"missing"}else{"imported"},c.now,SCHEDULER_VERSION]).map_err(unavailable)?;
                        report.cards_added+=1;
                    }
                }
            }
            let report_json=serde_json::to_value(&report).map_err(unavailable)?;
            tx.execute("UPDATE imports SET report_json=?1 WHERE id=?2",params![report_json.to_string(),c.import_id]).map_err(unavailable)?;
            Ok((report_json.clone(),vec![Event{id:c.event_id,kind:"import_complete".into(),schema_version:1,payload:json!({"importId":c.import_id,"bookId":book,"sourceHash":c.source_hash,"counts":report_json})}]))
        })?;
        serde_json::from_value(result).map_err(unavailable)
    }
    pub fn cards(&self, profile: &str, filter: CardFilter) -> Result<CardPage> {
        if !(1..=500).contains(&filter.page_size) {
            return Err(invalid("分页大小必须为1–500"));
        }
        let db = self.connection.lock().map_err(unavailable)?;
        let state = filter.state.map(state_name);
        let predicate = "c.profile_id=?1 AND (?2 IS NULL OR EXISTS(SELECT 1 FROM book_entries e WHERE e.profile_id=c.profile_id AND e.lexeme_id=c.lexeme_id AND e.book_id=?2)) AND (?3='' OR instr(lower(c.front||' '||c.meaning),lower(?3))>0) AND (?4 IS NULL OR c.state=?4) AND (?5 IS NULL OR c.suspended=?5)";
        let total: i64 = db
            .query_row(
                &format!("SELECT count(*) FROM cards c WHERE {predicate}"),
                params![
                    profile,
                    filter.book_id,
                    filter.search,
                    state,
                    filter.suspended
                ],
                |r| r.get(0),
            )
            .map_err(unavailable)?;
        let mut stmt=db.prepare(&format!("SELECT c.* FROM cards c WHERE {predicate} ORDER BY c.created_at,c.id LIMIT ?6 OFFSET ?7")).map_err(unavailable)?;
        let cards = stmt
            .query_map(
                params![
                    profile,
                    filter.book_id,
                    filter.search,
                    state,
                    filter.suspended,
                    filter.page_size,
                    i64::from(filter.page) * i64::from(filter.page_size)
                ],
                read_card,
            )
            .map_err(unavailable)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(unavailable)?;
        for card in &cards {
            card.srs.validate().map_err(unavailable)?;
        }
        Ok(CardPage {
            total: total as u64,
            cards,
        })
    }
    pub fn review_queue(&self, profile: &str, now: i64) -> Result<(Vec<CardView>, bool)> {
        let (settings, _) = self.settings(profile)?;
        let day = study_day(
            now,
            settings.timezone.parse().map_err(unavailable)?,
            settings.day_start_hour,
        )
        .map_err(unavailable)?;
        let db = self.connection.lock().map_err(unavailable)?;
        let new_count:u32=db.query_row("SELECT count(DISTINCT card_id) FROM review_logs WHERE profile_id=?1 AND state_before='new' AND reviewed_at>=?2 AND reviewed_at<?3",params![profile,day.start_ms,day.end_ms],|r|r.get(0)).map_err(unavailable)?;
        let review_count:u32=db.query_row("SELECT count(*) FROM review_logs WHERE profile_id=?1 AND state_before='review' AND reviewed_at>=?2 AND reviewed_at<?3",params![profile,day.start_ms,day.end_ms],|r|r.get(0)).map_err(unavailable)?;
        let mut statement=db.prepare("SELECT * FROM cards WHERE profile_id=?1 AND suspended=0 AND archived_at IS NULL AND due<=?2").map_err(unavailable)?;
        let cards = statement
            .query_map(params![profile, now], read_card)
            .map_err(unavailable)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(unavailable)?;
        let entries = cards
            .iter()
            .map(|c| {
                Ok(srs::QueueCard {
                    id: c.id.clone().try_into().map_err(unavailable)?,
                    srs: c.srs.clone(),
                    created_at: c.created_at,
                    suspended: c.suspended,
                    archived: c.archived,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let order = srs::queue(
            &entries,
            now,
            srs::Quota {
                new_remaining: settings.daily_new_limit.saturating_sub(new_count),
                review_remaining: settings.daily_review_limit.saturating_sub(review_count),
            },
        );
        let mut by_id: std::collections::HashMap<_, _> =
            cards.into_iter().map(|c| (c.id.clone(), c)).collect();
        let ordered = order
            .into_iter()
            .filter_map(|id| by_id.remove(id.as_str()))
            .collect::<Vec<_>>();
        let waiting=ordered.is_empty()&&db.query_row("SELECT EXISTS(SELECT 1 FROM cards WHERE profile_id=?1 AND suspended=0 AND archived_at IS NULL AND state IN ('learning','relearning') AND due>?2)",params![profile,now],|r|r.get::<_,bool>(0)).map_err(unavailable)?;
        Ok((ordered, waiting))
    }

    pub fn rate(&self, c: RateCard) -> Result<CardView> {
        if !c.revealed {
            return Err(invalid("请先显示答案"));
        }
        let expected = i64::try_from(c.expected_revision).map_err(|_| invalid("版本无效"))?;
        let request=json!({"card":c.card_id,"revision":c.expected_revision,"rating":c.rating,"duration":c.duration_ms}).to_string();
        let result=self.command(&c.profile_id,&c.operation_id,"rate_card",&request,c.now,|tx|{
            let mut card=tx.query_row("SELECT * FROM cards WHERE profile_id=?1 AND id=?2",params![c.profile_id,c.card_id],read_card).optional().map_err(unavailable)?.ok_or(AppError::new(ErrorCode::NotFound,"词卡不存在"))?;
            if card.revision!=c.expected_revision{return Err(AppError::new(ErrorCode::StaleState,"词卡已变更，请重新加载"));}
            if card.suspended||card.archived{return Err(invalid("词卡已暂停或归档"));}
            let settings_json:String=tx.query_row("SELECT json FROM settings WHERE profile_id=?1",[&c.profile_id],|r|r.get(0)).map_err(unavailable)?;
            let settings:Settings=serde_json::from_str(&settings_json).map_err(unavailable)?;
            let settings=ValidatedSettings::try_from(settings).map_err(unavailable)?;let settings=settings.get();
            let day=study_day(c.now,settings.timezone.parse().map_err(unavailable)?,settings.day_start_hour).map_err(unavailable)?;
            if matches!(card.srs.state,State::New|State::Review){
                let (query,limit)=if card.srs.state==State::New{("SELECT count(DISTINCT card_id) FROM review_logs WHERE profile_id=?1 AND state_before='new' AND reviewed_at>=?2 AND reviewed_at<?3",settings.daily_new_limit)}else{("SELECT count(*) FROM review_logs WHERE profile_id=?1 AND state_before='review' AND reviewed_at>=?2 AND reviewed_at<?3",settings.daily_review_limit)};
                let count:u32=tx.query_row(query,params![c.profile_id,day.start_ms,day.end_ms],|r|r.get(0)).map_err(unavailable)?;
                if count>=limit{return Err(AppError::new(ErrorCode::QuotaExceeded,"今日该类复习额度已用完"));}
            }
            let before=card.srs.clone();let next=srs::rate(&before,c.rating,c.now,settings.request_retention).map_err(|e|invalid(&e.to_string()))?;
            let changed=tx.execute("UPDATE cards SET state=?1,due=?2,stability=?3,difficulty=?4,reps=?5,lapses=?6,last_review=?7,elapsed_days=?8,scheduled_days=?9,learning_steps=?10,scheduler_version=?11,updated_at=?12,revision=revision+1 WHERE profile_id=?13 AND id=?14 AND revision=?15",params![state_name(next.state),next.due,next.stability,next.difficulty,next.reps,next.lapses,next.last_review,next.elapsed_days,next.scheduled_days,next.learning_steps,SCHEDULER_VERSION,c.now,c.profile_id,c.card_id,expected]).map_err(unavailable)?;
            if changed!=1{return Err(AppError::new(ErrorCode::StaleState,"词卡已变更"));}
            tx.execute("INSERT INTO review_logs(id,profile_id,card_id,operation_id,rating,state_before,state_after,due_before,due_after,scheduler_version,reviewed_at,duration_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",params![c.review_id,c.profile_id,c.card_id,c.operation_id,serde_json::to_value(c.rating).map_err(unavailable)?.as_str(),state_name(before.state),state_name(next.state),before.due,next.due,SCHEDULER_VERSION,c.now,c.duration_ms]).map_err(unavailable)?;
            card.srs=next;card.revision+=1;
            Ok((serde_json::to_value(&card).map_err(unavailable)?,vec![Event{id:c.event_id,kind:"review".into(),schema_version:1,payload:json!({"cardId":c.card_id,"rating":c.rating,"schedulerVersion":SCHEDULER_VERSION})}]))
        })?;
        serde_json::from_value(result).map_err(unavailable)
    }
}
impl LibraryRepository for Store {
    async fn import_words(&self, c: ImportWords) -> Result<ImportReport> {
        self.import(c)
    }
    async fn query_cards(&self, profile: &str, filter: CardFilter) -> Result<CardPage> {
        self.cards(profile, filter)
    }
    async fn rate_card(&self, c: RateCard) -> Result<CardView> {
        self.rate(c)
    }
}
