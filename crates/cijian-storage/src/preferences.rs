use super::*;
use cijian_domain::settings::DevicePreferences;
use cijian_ports::SaveDevicePreferences;
impl Store {
    pub fn ensure_device(&self, profile: &str, device: &str, platform: &str) -> Result<()> {
        let (legacy, _) = self.settings(profile)?;
        let mut db = self.connection.lock().map_err(unavailable)?;
        let tx = db.transaction().map_err(unavailable)?;
        tx.execute(
            "INSERT OR IGNORE INTO devices(profile_id,id,platform) VALUES (?1,?2,?3)",
            params![profile, device, platform],
        )
        .map_err(unavailable)?;
        let value = DevicePreferences {
            theme: legacy.theme,
            playback_rate: legacy.playback_rate,
            system_voice: None,
        };
        tx.execute("INSERT OR IGNORE INTO device_preferences(profile_id,device_id,theme,playback_rate,json,revision) VALUES (?1,?2,?3,?4,?5,1)",params![profile,device,serde_json::to_string(&value.theme).map_err(unavailable)?,value.playback_rate,serde_json::to_string(&value).map_err(unavailable)?]).map_err(unavailable)?;
        tx.commit().map_err(unavailable)
    }
    pub fn device_preferences(
        &self,
        profile: &str,
        device: &str,
    ) -> Result<(DevicePreferences, u64)> {
        let db = self.connection.lock().map_err(unavailable)?;
        let (json, revision): (String, i64) = db
            .query_row(
                "SELECT json,revision FROM device_preferences WHERE profile_id=?1 AND device_id=?2",
                params![profile, device],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(unavailable)?;
        let value: DevicePreferences = serde_json::from_str(&json).map_err(unavailable)?;
        value.validate().map_err(|e| invalid(&e.to_string()))?;
        let revision = u64::try_from(revision)
            .ok()
            .filter(|v| *v >= 1)
            .ok_or(invalid("设备设置版本无效"))?;
        Ok((value, revision))
    }
    pub fn save_device_preferences(&self, c: SaveDevicePreferences) -> Result<u64> {
        c.value.validate().map_err(|e| invalid(&e.to_string()))?;
        let revision = c
            .expected_revision
            .checked_add(1)
            .filter(|v| *v <= i64::MAX as u64)
            .ok_or(invalid("设备设置版本超出范围"))?;
        let request =
            serde_json::json!({"device":c.device,"revision":c.expected_revision,"value":c.value})
                .to_string();
        self.command(&c.profile,&c.operation,"device_preferences_update",&request,c.now,|tx|{
            let count=tx.execute("UPDATE device_preferences SET theme=?1,playback_rate=?2,json=?3,revision=?4 WHERE profile_id=?5 AND device_id=?6 AND revision=?7",params![serde_json::to_string(&c.value.theme).map_err(unavailable)?,c.value.playback_rate,serde_json::to_string(&c.value).map_err(unavailable)?,revision as i64,c.profile,c.device,c.expected_revision as i64]).map_err(unavailable)?;
            if count!=1{return Err(AppError::new(ErrorCode::StaleState,"本机偏好已改变，请重新加载"));}
            Ok((serde_json::json!(revision),vec![Event{id:c.event,kind:"device_preferences_update".into(),schema_version:1,payload:serde_json::json!({"deviceId":c.device,"revision":revision})}]))
        })?.as_u64().ok_or(invalid("设备设置回执无效"))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_device_preserves_older_legal_playback_rates() {
        for rate in [0.9, 1.1] {
            let store = Store::memory().unwrap();
            store.create_profile("p", "测试", 0).unwrap();
            let mut legacy = store.settings("p").unwrap().0;
            legacy.playback_rate = rate;
            legacy.theme = cijian_domain::settings::Theme::Dark;
            store
                .connection
                .lock()
                .unwrap()
                .execute(
                    "UPDATE settings SET json=?1 WHERE profile_id='p'",
                    [serde_json::to_string(&legacy).unwrap()],
                )
                .unwrap();
            store.ensure_device("p", "first", "test").unwrap();
            let (value, _) = store.device_preferences("p", "first").unwrap();
            assert_eq!(value.playback_rate, rate);
            assert_eq!(value.theme, cijian_domain::settings::Theme::Dark);
        }
    }
    #[test]
    fn preferences_are_device_local_and_event_failure_rolls_back() {
        let store = Store::memory().unwrap();
        store.create_profile("p", "测试", 0).unwrap();
        store.ensure_device("p", "a", "test").unwrap();
        store.ensure_device("p", "b", "test").unwrap();
        let value = DevicePreferences {
            theme: cijian_domain::settings::Theme::Dark,
            playback_rate: 1.25,
            system_voice: None,
        };
        let command = |event: &str, operation: &str| SaveDevicePreferences {
            profile: "p".into(),
            device: "a".into(),
            event: event.into(),
            operation: operation.into(),
            expected_revision: 1,
            value: value.clone(),
            now: 1,
        };
        assert!(
            store
                .save_device_preferences(command("bootstrap-p", "bad"))
                .is_err()
        );
        assert_eq!(store.device_preferences("p", "a").unwrap().1, 1);
        assert_eq!(
            store
                .save_device_preferences(command("good", "op"))
                .unwrap(),
            2
        );
        assert_eq!(
            store
                .save_device_preferences(command("good", "op"))
                .unwrap(),
            2
        );
        assert_eq!(store.device_preferences("p", "a").unwrap().0, value);
        assert_eq!(
            store.device_preferences("p", "b").unwrap().0,
            DevicePreferences::default()
        );
        assert_eq!(
            store.settings("p").unwrap().0.theme,
            cijian_domain::settings::Theme::System
        );
    }
}
