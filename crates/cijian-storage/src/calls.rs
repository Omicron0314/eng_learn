use super::{Store, unavailable};
use cijian_ports::{
    AppError, ErrorCode, Result,
    provider::{CallFinish, CallRecorder, CallStart, CallStatus},
};
use rusqlite::params;
impl Store {
    pub fn start_call(&self, call: CallStart) -> Result<()> {
        if call.call_id.is_empty() || call.profile_id.is_empty() {
            return Err(AppError::new(ErrorCode::InvalidInput, "调用标识无效"));
        }
        self.connection.lock().map_err(unavailable)?.execute("INSERT INTO llm_calls(id,profile_id,purpose,provider,model,prompt_version,latency_ms,status,ts) VALUES (?1,?2,?3,?4,?5,?6,0,'started',?7)",params![call.call_id,call.profile_id,call.purpose,call.provider_fingerprint,call.model,call.prompt_version,call.now]).map_err(unavailable)?;
        Ok(())
    }
    pub fn finish_call(&self, call: CallFinish) -> Result<()> {
        let number = |v: u64| i64::try_from(v).map_err(unavailable);
        let status = match call.status {
            CallStatus::Success => "success",
            CallStatus::TransportError => "transport_error",
            CallStatus::HttpError => "http_error",
            CallStatus::InvalidOutput => "invalid_output",
            CallStatus::Interrupted => "interrupted",
        };
        let changed=self.connection.lock().map_err(unavailable)?.execute("UPDATE llm_calls SET status=?1,tokens_in=?2,tokens_out=?3,latency_ms=?4 WHERE id=?5 AND status='started'",params![status,call.tokens_in.map(number).transpose()?,call.tokens_out.map(number).transpose()?,number(call.latency_ms)?,call.call_id]).map_err(unavailable)?;
        if changed != 1 {
            return Err(AppError::new(
                ErrorCode::StaleState,
                "调用记录不存在或已结束",
            ));
        }
        Ok(())
    }
}
impl CallRecorder for Store {
    async fn start(&self, call: CallStart) -> Result<()> {
        self.start_call(call)
    }
    async fn finish(&self, call: CallFinish) -> Result<()> {
        self.finish_call(call)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn started_call_survives_failure_to_finalize_and_cost_is_unknown() {
        let store = Store::memory().unwrap();
        store.create_profile("p", "测试", 0).unwrap();
        store
            .start_call(CallStart {
                profile_id: "p".into(),
                call_id: "c".into(),
                purpose: "article".into(),
                model: "configured".into(),
                prompt_version: "v1".into(),
                provider_fingerprint: "hash".into(),
                now: 0,
            })
            .unwrap();
        assert_eq!(
            store
                .connection
                .lock()
                .unwrap()
                .query_row("SELECT status FROM llm_calls", [], |r| r
                    .get::<_, String>(0))
                .unwrap(),
            "started"
        );
        store
            .finish_call(CallFinish {
                call_id: "c".into(),
                status: CallStatus::TransportError,
                tokens_in: None,
                tokens_out: None,
                latency_ms: 20,
            })
            .unwrap();
        let result = store
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT status,cost_usd,tokens_in FROM llm_calls", [], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<f64>>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                ))
            })
            .unwrap();
        assert_eq!(result, ("transport_error".into(), None, None));
    }
}
