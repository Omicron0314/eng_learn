use super::*;
use cijian_domain::provider::{ProviderConfig, ProviderKind};
use serde_json::json;
fn kind(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Llm => "llm",
        ProviderKind::Tts => "tts",
    }
}
impl Store {
    pub fn provider_config(
        &self,
        profile: &str,
        provider_kind: ProviderKind,
    ) -> Result<Option<ProviderConfig>> {
        let db = self.connection.lock().map_err(unavailable)?;
        let result=db.query_row("SELECT endpoint,model,voice,revision FROM provider_configs WHERE profile_id=?1 AND kind=?2",params![profile,kind(provider_kind)],|r|Ok(ProviderConfig{kind:provider_kind,endpoint:r.get(0)?,model:r.get(1)?,voice:r.get(2)?,revision:u64::try_from(r.get::<_,i64>(3)?).map_err(|_|rusqlite::Error::IntegralValueOutOfRange(3,-1))?})).optional().map_err(unavailable)?;
        if let Some(config) = &result {
            config.validate().map_err(unavailable)?;
        }
        Ok(result)
    }
    pub fn save_provider_config(
        &self,
        c: cijian_ports::provider::SaveProviderConfig,
    ) -> Result<ProviderConfig> {
        let cijian_ports::provider::SaveProviderConfig {
            profile,
            operation,
            event,
            config,
            expected_revision,
            now,
        } = c;
        let profile = profile.as_str();
        let operation = operation.as_str();
        let event = event.as_str();
        config.validate().map_err(|e| invalid(&e.to_string()))?;
        if config.revision
            != expected_revision
                .checked_add(1)
                .ok_or(invalid("配置版本超出范围"))?
            || config.revision > i64::MAX as u64
        {
            return Err(invalid("配置版本无效"));
        }
        let request = serde_json::to_string(&config).map_err(unavailable)?;
        let value=self.command(profile,operation,"save_provider_config",&request,now,|tx|{
            let current:Option<i64>=tx.query_row("SELECT revision FROM provider_configs WHERE profile_id=?1 AND kind=?2",params![profile,kind(config.kind)],|r|r.get(0)).optional().map_err(unavailable)?;
            if current.unwrap_or(0)!=expected_revision as i64{return Err(AppError::new(ErrorCode::StaleState,"服务配置已改变，请重新加载"));}
            tx.execute("INSERT INTO provider_configs VALUES (?1,?2,?3,?4,?5,?6,NULL,?7) ON CONFLICT(profile_id,kind) DO UPDATE SET endpoint=excluded.endpoint,model=excluded.model,voice=excluded.voice,revision=excluded.revision",params![format!("{profile}/{}",kind(config.kind)),profile,kind(config.kind),config.endpoint,config.model,config.voice,config.revision as i64]).map_err(unavailable)?;
            Ok((serde_json::to_value(&config).map_err(unavailable)?,vec![Event{id:event.into(),kind:"provider_update".into(),schema_version:1,payload:json!({"kind":kind(config.kind),"configRevision":config.revision,"changedKeys":["endpoint","model","voice"]})}]))
        })?;
        serde_json::from_value(value).map_err(unavailable)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versioned_configuration_no_keys_or_network() {
        let store = Store::memory().unwrap();
        store.create_profile("p", "test", 0).unwrap();
        assert!(
            store
                .provider_config("p", ProviderKind::Llm)
                .unwrap()
                .is_none()
        );
        let config = ProviderConfig {
            kind: ProviderKind::Llm,
            endpoint: "https://example.invalid/v1".into(),
            model: "chosen".into(),
            voice: None,
            revision: 1,
        };
        let command = |op: &str| cijian_ports::provider::SaveProviderConfig {
            profile: "p".into(),
            operation: op.into(),
            event: format!("event-{op}"),
            config: config.clone(),
            expected_revision: 0,
            now: 1,
        };
        store.save_provider_config(command("save")).unwrap();
        store.save_provider_config(command("save")).unwrap();
        assert_eq!(
            store
                .save_provider_config(command("stale"))
                .unwrap_err()
                .code,
            ErrorCode::StaleState
        );
        assert_eq!(
            store
                .provider_config("p", ProviderKind::Llm)
                .unwrap()
                .unwrap()
                .model,
            "chosen"
        );
    }
}
