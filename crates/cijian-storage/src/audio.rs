#[cfg(test)]
mod tests;
use super::*;
use cijian_domain::time::study_day;
use cijian_ports::audio::*;
use serde_json::json;
impl Store {
    pub fn audio_asset(&self, profile: &str, cache_key: &str) -> Result<Option<AudioAsset>> {
        let db = self.connection.lock().map_err(unavailable)?;
        let value:Option<String>=db.query_row("SELECT json_object('id',id,'cache_key',cache_key,'text_hash',text_hash,'provider_fingerprint',provider_fingerprint,'model',model,'voice',voice,'format',format,'blob_key',blob_key,'duration',duration,'chars',chars) FROM audio_assets WHERE profile_id=?1 AND cache_key=?2",params![profile,cache_key],|r|r.get(0)).optional().map_err(unavailable)?;
        value
            .map(|v| serde_json::from_str(&v).map_err(unavailable))
            .transpose()
    }
    pub fn link_cached_audio(&self, profile: &str, article: &str, audio: &str) -> Result<()> {
        let db = self.connection.lock().map_err(unavailable)?;
        let matches:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM articles a JOIN audio_assets s ON s.profile_id=a.profile_id AND s.text_hash=a.body_hash WHERE a.profile_id=?1 AND a.id=?2 AND s.id=?3)",params![profile,article,audio],|r|r.get(0)).map_err(unavailable)?;
        if !matches {
            return Err(invalid("音频与正文不匹配"));
        }
        db.execute(
            "INSERT OR IGNORE INTO article_audio_links VALUES (?1,?2,?3)",
            params![profile, article, audio],
        )
        .map_err(unavailable)?;
        Ok(())
    }
    pub fn reserve_audio(&self, c: ReserveAudio) -> Result<AudioReservation> {
        let request = json!({"article":c.article,"bodyHash":c.body_hash}).to_string();
        let value=self.command(&c.profile,&c.operation,"reserve_audio",&request,c.now,|tx|{
            let article:Option<(String,String)>=tx.query_row("SELECT body,body_hash FROM articles WHERE profile_id=?1 AND id=?2",params![c.profile,c.article],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(unavailable)?;
            let (text,body_hash)=article.ok_or(AppError::new(ErrorCode::NotFound,"文章不存在"))?;
            if c.body_hash!=body_hash{return Err(AppError::new(ErrorCode::StaleState,"正文已变化"));}
            let chars=i64::try_from(text.chars().count()).map_err(|_|invalid("正文过长"))?;
            let settings:String=tx.query_row("SELECT json FROM settings WHERE profile_id=?1",[&c.profile],|r|r.get(0)).map_err(unavailable)?;
            let settings:Settings=serde_json::from_str(&settings).map_err(unavailable)?;let settings=ValidatedSettings::try_from(settings).map_err(unavailable)?;let settings=settings.get();
            let day=study_day(c.now,settings.timezone.parse().map_err(unavailable)?,settings.day_start_hour).map_err(unavailable)?;
            let used:i64=tx.query_row("SELECT COALESCE(sum(chars),0) FROM audio_reservations WHERE profile_id=?1 AND reserved_at>=?2 AND reserved_at<?3",params![c.profile,day.start_ms,day.end_ms],|r|r.get(0)).map_err(unavailable)?;
            if chars<=0||used.checked_add(chars).is_none_or(|sum|sum>i64::from(settings.tts_daily_char_limit)){return Err(AppError::new(ErrorCode::QuotaExceeded,"今日远程朗读字符额度不足"));}
            tx.execute("INSERT INTO audio_reservations VALUES (?1,?2,?3,?4,?5,?6,'reserved')",params![c.reservation,c.profile,c.operation,c.article,chars,c.now]).map_err(unavailable)?;
            let result=AudioReservation{id:c.reservation.clone(),article:c.article.clone(),chars:chars as u64,outcome:"reserved".into()};
            Ok((serde_json::to_value(result).map_err(unavailable)?,vec![Event{id:c.event,kind:"audio_reserve".into(),schema_version:1,payload:json!({"reservationId":c.reservation,"articleId":c.article,"chars":chars})}]))
        })?;
        serde_json::from_value(value).map_err(unavailable)
    }
    pub fn claim_audio(&self, profile: &str, reservation: &str) -> Result<bool> {
        let db = self.connection.lock().map_err(unavailable)?;
        Ok(db.execute("UPDATE audio_reservations SET outcome='started' WHERE profile_id=?1 AND id=?2 AND outcome='reserved'",params![profile,reservation]).map_err(unavailable)?==1)
    }
    /// Failure records retain the original reservation; quota is never refunded.
    pub fn fail_audio(&self, profile: &str, reservation: &str) -> Result<()> {
        self.connection.lock().map_err(unavailable)?.execute("UPDATE audio_reservations SET outcome='failed' WHERE profile_id=?1 AND id=?2 AND outcome IN ('reserved','started')",params![profile,reservation]).map_err(unavailable)?;
        Ok(())
    }
    pub fn save_audio(&self, c: SaveAudio) -> Result<AudioAsset> {
        if c.asset.cache_key.is_empty()
            || c.asset.blob_key.is_empty()
            || c.asset.model.trim().is_empty()
            || c.asset.voice.trim().is_empty()
            || !["mp3", "wav", "opus", "flac"].contains(&c.asset.format.as_str())
            || c.asset.duration.is_some_and(|d| !d.is_finite() || d <= 0.)
        {
            return Err(invalid("音频元数据无效"));
        }
        let request = json!({"reservation":c.reservation,"asset":c.asset}).to_string();
        let value=self.command(&c.profile,&c.operation,"save_audio",&request,c.now,|tx|{
            let reservation:Option<(String,i64)>=tx.query_row("SELECT article_id,chars FROM audio_reservations WHERE profile_id=?1 AND id=?2 AND outcome='started'",params![c.profile,c.reservation],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(unavailable)?;
            let (article,chars)=reservation.ok_or(AppError::new(ErrorCode::StaleState,"音频任务已结束或未领取"))?;
            let text_hash:String=tx.query_row("SELECT body_hash FROM articles WHERE profile_id=?1 AND id=?2",params![c.profile,article],|r|r.get(0)).map_err(unavailable)?;
            if c.asset.chars!=chars as u64||c.asset.text_hash!=text_hash{return Err(invalid("音频与预留正文不匹配"));}
            tx.execute("INSERT INTO audio_assets VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12) ON CONFLICT(cache_key) DO UPDATE SET blob_key=excluded.blob_key,duration=excluded.duration,created_at=excluded.created_at WHERE audio_assets.profile_id=excluded.profile_id AND audio_assets.text_hash=excluded.text_hash AND audio_assets.provider_fingerprint=excluded.provider_fingerprint",params![c.asset.id,c.profile,c.asset.cache_key,c.asset.text_hash,c.asset.provider_fingerprint,c.asset.model,c.asset.voice,c.asset.format,c.asset.blob_key,c.asset.duration,chars,c.now]).map_err(unavailable)?;
            let actual_id:String=tx.query_row("SELECT id FROM audio_assets WHERE profile_id=?1 AND cache_key=?2",params![c.profile,c.asset.cache_key],|r|r.get(0)).map_err(unavailable)?;
            let mut asset=c.asset.clone();asset.id=actual_id;
            tx.execute("INSERT OR IGNORE INTO article_audio_links VALUES (?1,?2,?3)",params![c.profile,article,asset.id]).map_err(unavailable)?;
            tx.execute("UPDATE audio_reservations SET outcome='succeeded' WHERE profile_id=?1 AND id=?2",params![c.profile,c.reservation]).map_err(unavailable)?;
            Ok((serde_json::to_value(&asset).map_err(unavailable)?,vec![Event{id:c.event,kind:"audio_generate".into(),schema_version:1,payload:json!({"reservationId":c.reservation,"articleId":article,"audioId":asset.id,"chars":chars})}]))
        })?;
        serde_json::from_value(value).map_err(unavailable)
    }
    pub fn open_audio_session(&self, c: AudioSessionCommand) -> Result<()> {
        if !["remote", "system", "browser"].contains(&c.source.as_str())
            || ![0.75, 1., 1.25, 1.5, 2.].contains(&c.rate)
            || (c.source == "remote" && c.audio.is_none())
        {
            return Err(invalid("音频会话参数无效"));
        }
        let request=json!({"session":c.session,"article":c.article,"audio":c.audio,"source":c.source,"rate":c.rate}).to_string();
        self.command(&c.profile,&c.operation,"audio_play",&request,c.now,|tx|{
            if let Some(audio)=&c.audio{let linked:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM article_audio_links WHERE profile_id=?1 AND article_id=?2 AND audio_id=?3)",params![c.profile,c.article,audio],|r|r.get(0)).map_err(unavailable)?;if !linked{return Err(invalid("音频不属于该文章"));}}
            tx.execute("INSERT INTO audio_sessions VALUES (?1,?2,?3,?4,?5,?6,NULL,?7)",params![c.session,c.profile,c.article,c.audio,c.source,c.now,c.rate]).map_err(unavailable)?;
            Ok((json!(true),vec![Event{id:c.event,kind:"audio_play".into(),schema_version:1,payload:json!({"articleId":c.article,"sessionId":c.session,"source":c.source,"rate":c.rate})}]))
        })?;
        Ok(())
    }
    pub fn complete_audio_session(&self, c: AudioSessionCommand) -> Result<()> {
        let request = json!({"session":c.session,"article":c.article}).to_string();
        self.command(&c.profile,&c.operation,"audio_complete",&request,c.now,|tx|{
            let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM audio_sessions WHERE profile_id=?1 AND article_id=?2 AND id=?3)",params![c.profile,c.article,c.session],|r|r.get(0)).map_err(unavailable)?;
            if !exists{return Err(AppError::new(ErrorCode::NotFound,"播放会话不存在"));}
            let changed=tx.execute("UPDATE audio_sessions SET completed_at=?1 WHERE profile_id=?2 AND id=?3 AND completed_at IS NULL",params![c.now,c.profile,c.session]).map_err(unavailable)?;
            Ok((json!(true),vec![Event{id:c.event,kind:if changed==1{"audio_complete"}else{"audio_complete_replayed"}.into(),schema_version:1,payload:json!({"articleId":c.article,"sessionId":c.session})}]))
        })?;
        Ok(())
    }
}
