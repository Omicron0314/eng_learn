#[cfg(test)]
mod tests;
use super::*;
use cijian_ports::audio::*;
use serde_json::json;
impl Store {
    pub fn article_audio_assets(&self, profile: &str, article: &str) -> Result<Vec<AudioAsset>> {
        let db = self.connection.lock().map_err(unavailable)?;
        let mut statement=db.prepare("SELECT json_object('id',s.id,'cache_key',s.cache_key,'text_hash',s.text_hash,'provider_fingerprint',s.provider_fingerprint,'model',s.model,'voice',s.voice,'format',s.format,'blob_key',s.blob_key,'duration',s.duration,'chars',s.chars) FROM audio_assets s JOIN article_audio_links l ON l.audio_id=s.id AND l.profile_id=s.profile_id WHERE l.profile_id=?1 AND l.article_id=?2 ORDER BY s.created_at DESC,s.id").map_err(unavailable)?;
        statement
            .query_map(params![profile, article], |r| r.get::<_, String>(0))
            .map_err(unavailable)?
            .map(|value| serde_json::from_str(&value.map_err(unavailable)?).map_err(unavailable))
            .collect()
    }
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
    /// Local synthesis has no paid reservation. Publish the complete asset and
    /// article link together with its event, after the blob is durably written.
    pub fn save_local_audio(&self, c: SaveLocalAudio) -> Result<AudioAsset> {
        if c.asset.id.is_empty()
            || c.asset.cache_key.is_empty()
            || c.asset.model.is_empty()
            || c.asset.voice.is_empty()
            || c.asset.provider_fingerprint.is_empty()
            || c.asset.format != "wav"
            || !c.asset.duration.is_some_and(|d| d.is_finite() && d > 0.)
            || c.asset.blob_key.len() != 68
            || !c.asset.blob_key.ends_with(".wav")
            || !c.asset.blob_key.as_bytes()[..64]
                .iter()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err(invalid("本地音频元数据无效"));
        }
        let request = json!({"article":c.article,"asset":c.asset}).to_string();
        let value = self.command(&c.profile, &c.operation, "save_local_audio", &request, c.now, |tx| {
            let row: Option<(String, String)> = tx.query_row(
                "SELECT body,body_hash FROM articles WHERE profile_id=?1 AND id=?2",
                params![c.profile,c.article], |r| Ok((r.get(0)?,r.get(1)?))
            ).optional().map_err(unavailable)?;
            let (body, hash) = row.ok_or(AppError::new(ErrorCode::NotFound,"文章不存在"))?;
            if hash != c.asset.text_hash || body.chars().count() as u64 != c.asset.chars {
                return Err(AppError::new(ErrorCode::StaleState,"正文已变化，请重新生成音频"));
            }
            tx.execute("INSERT INTO audio_assets VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12) ON CONFLICT(cache_key) DO UPDATE SET blob_key=excluded.blob_key,duration=excluded.duration,created_at=excluded.created_at WHERE audio_assets.profile_id=excluded.profile_id AND audio_assets.text_hash=excluded.text_hash AND audio_assets.provider_fingerprint=excluded.provider_fingerprint", params![c.asset.id,c.profile,c.asset.cache_key,c.asset.text_hash,c.asset.provider_fingerprint,c.asset.model,c.asset.voice,c.asset.format,c.asset.blob_key,c.asset.duration,body.chars().count() as i64,c.now]).map_err(unavailable)?;
            let actual_id: String = tx.query_row("SELECT id FROM audio_assets WHERE profile_id=?1 AND cache_key=?2 AND text_hash=?3 AND provider_fingerprint=?4", params![c.profile,c.asset.cache_key,c.asset.text_hash,c.asset.provider_fingerprint], |r| r.get(0)).map_err(unavailable)?;
            let mut asset = c.asset.clone();
            asset.id = actual_id;
            tx.execute("INSERT OR IGNORE INTO article_audio_links VALUES (?1,?2,?3)",params![c.profile,c.article,asset.id]).map_err(unavailable)?;
            Ok((serde_json::to_value(&asset).map_err(unavailable)?,vec![Event{id:c.event,kind:"audio_generate".into(),schema_version:1,payload:json!({"articleId":c.article,"audioId":asset.id,"source":"kokoro","chars":asset.chars})}]))
        })?;
        serde_json::from_value(value).map_err(unavailable)
    }
    pub fn open_audio_session(&self, c: AudioSessionCommand) -> Result<()> {
        if !["remote", "system", "browser"].contains(&c.source.as_str())
            || !cijian_domain::settings::PLAYBACK_RATES.contains(&c.rate)
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
