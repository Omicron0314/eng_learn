use super::*;
fn article_exists(tx: &Connection, profile: &str, article: Option<&str>) -> Result<bool> {
    tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM articles WHERE profile_id=?1 AND id=?2)",
        params![profile, article],
        |r| r.get(0),
    )
    .map_err(unavailable)
}
fn charge(
    tx: &Connection,
    profile: &str,
    id: &str,
    article: Option<&str>,
    chars: i64,
    at: i64,
    outcome: &str,
) -> Result<()> {
    if chars < 0 {
        return Err(invalid("旧音频字符数为负"));
    }
    insert(
        tx,
        "audio_reservations",
        json!({"id":id,"profile_id":profile,"operation_id":format!("legacy-charge-{id}"),"article_id":article,"chars":chars,"reserved_at":at,"outcome":outcome}),
    )
}
impl Store {
    pub(super) fn project_legacy_events(
        &self,
        tx: &Connection,
        snapshot: &LegacySnapshot,
        _settings: &Settings,
        report: &mut LegacyReport,
    ) -> Result<()> {
        let profile = &snapshot.report.profile;
        let mut events = snapshot.rows["events"].iter().collect::<Vec<_>>();
        events.sort_by_key(|value| (value["ts"].as_i64(), value["id"].as_str()));
        let mut reservations = BTreeSet::new();
        let mut charged_audio = BTreeSet::new();
        for event in &events {
            let id = text(event, "id")?;
            let kind = text(event, "type")?;
            let payload = parsed(event, "payload_json")?;
            insert(
                tx,
                "events",
                json!({"id":id,"profile_id":profile,"device_id":null,"operation_id":format!("legacy-event-{id}"),"ordinal":0,"type":kind,"schema_version":0,"payload_json":event["payload_json"],"occurred_at":event["ts"]}),
            )?;
            if kind == "audio_reserve" {
                let article = payload["articleId"].as_str();
                let exists = article_exists(tx, profile, article)?;
                if !exists {
                    report
                        .warnings
                        .push(format!("旧预留 {id} 无可用文章引用；仍保留额度和原事件"));
                }
                charge(
                    tx,
                    profile,
                    id,
                    if exists { article } else { None },
                    number(&payload, "chars")?,
                    number(event, "ts")?,
                    "legacy-reserved",
                )?;
                reservations.insert(id.to_owned());
            }
        }
        for event in events {
            let id = text(event, "id")?;
            let kind = text(event, "type")?;
            let payload = parsed(event, "payload_json")?;
            let at = number(event, "ts")?;
            let article = payload["articleId"].as_str();
            let exists = article_exists(tx, profile, article)?;
            match kind {
                "article_open" if exists => insert(
                    tx,
                    "reading_sessions",
                    json!({"id":id,"profile_id":profile,"article_id":article,"opened_at":at,"completed_at":null,"position":0}),
                )?,
                "article_complete" if exists => {
                    let session = if let Some(open) = payload["openId"].as_str() {
                        Some(open.to_owned())
                    } else {
                        tx.query_row("SELECT id FROM reading_sessions WHERE profile_id=?1 AND article_id=?2 AND opened_at<=?3 ORDER BY opened_at DESC,id DESC LIMIT 1",params![profile,article,at],|r|r.get::<_,String>(0)).optional().map_err(unavailable)?
                    };
                    let changed=tx.execute("UPDATE reading_sessions SET completed_at=COALESCE(completed_at,?1) WHERE profile_id=?2 AND article_id=?3 AND id=?4 AND opened_at<=?1",params![at,profile,article,session]).map_err(unavailable)?;
                    if changed == 0 {
                        report.warnings.push(format!(
                            "旧阅读完成 {id} 缺少可证明的打开会话，仅保留原事件"
                        ));
                    }
                }
                "audio_play" if exists => {
                    let source = match payload["via"].as_str() {
                        Some("server") => "remote",
                        Some("browser") => "browser",
                        _ => {
                            report
                                .warnings
                                .push(format!("旧播放 {id} 来源未知，仅保留原事件"));
                            continue;
                        }
                    };
                    let rate = payload["rate"]
                        .as_f64()
                        .filter(|rate| rate.is_finite() && (0.75..=1.25).contains(rate))
                        .ok_or(invalid("旧播放速度无效"))?;
                    insert(
                        tx,
                        "audio_sessions",
                        json!({"id":id,"profile_id":profile,"article_id":article,"audio_id":null,"source":source,"started_at":at,"completed_at":null,"rate":rate}),
                    )?;
                }
                "audio_complete" => report.warnings.push(format!(
                    "旧完成 {id} 缺少自然结束会话合同，仅保留原事件，不推断听完"
                )),
                "audio_generate" => {
                    if let Some(audio) = payload["audioId"].as_str() {
                        charged_audio.insert(audio.to_owned());
                    }
                    if payload["reservationId"]
                        .as_str()
                        .is_some_and(|id| reservations.contains(id))
                    {
                        continue;
                    }
                    if !exists {
                        report
                            .warnings
                            .push(format!("旧合成 {id} 无可用文章引用；仍保守计入额度"));
                    }
                    charge(
                        tx,
                        profile,
                        &snapshot.new_ids[&format!("charge:event:{id}")],
                        if exists { article } else { None },
                        number(&payload, "chars")?,
                        at,
                        "legacy-success-without-reservation",
                    )?;
                }
                "article_open" | "article_complete" | "audio_play" => report
                    .warnings
                    .push(format!("旧事件 {id} 缺少当前文章，仅保留历史")),
                _ => {}
            }
        }
        for audio in &snapshot.rows["audios"] {
            let id = text(audio, "id")?;
            if !charged_audio.contains(id) {
                charge(
                    tx,
                    profile,
                    &snapshot.new_ids[&format!("charge:audio:{id}")],
                    audio["article_id"].as_str(),
                    number(audio, "chars")?,
                    number(audio, "created_at")?,
                    "legacy-unattributed-success",
                )?;
                report
                    .warnings
                    .push(format!("旧音频 {id} 缺少可关联成功事件，保守计入额度"));
            }
        }
        Ok(())
    }
}
