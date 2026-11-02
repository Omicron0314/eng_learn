#[cfg(test)]
pub(crate) mod tests;
use super::*;
use cijian_ports::articles::*;
use serde_json::json;
use std::collections::BTreeSet;
type JobRow = (String, String, String, Option<String>, Option<String>, i64);
type ArticleRow = (String, String, String, String, String, String, String, i64);
fn status(value: &str) -> Result<JobStatus> {
    match value {
        "queued" => Ok(JobStatus::Queued),
        "running" => Ok(JobStatus::Running),
        "succeeded" => Ok(JobStatus::Succeeded),
        "failed" => Ok(JobStatus::Failed),
        "cancelled" => Ok(JobStatus::Cancelled),
        "interrupted" => Ok(JobStatus::Interrupted),
        _ => Err(unavailable("invalid job state")),
    }
}
fn job(db: &Connection, profile: &str, id: &str) -> Result<GenerationJob> {
    let row:Option<JobRow>=db.query_row("SELECT id,operation_id,status,result_ref,error,config_revision FROM jobs WHERE profile_id=?1 AND id=?2",params![profile,id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional().map_err(unavailable)?;
    let (id, operation_id, state, result_ref, error, revision) =
        row.ok_or(AppError::new(ErrorCode::NotFound, "生成任务不存在"))?;
    Ok(GenerationJob {
        id,
        operation_id,
        status: status(&state)?,
        result_ref,
        error,
        config_revision: u64::try_from(revision).map_err(unavailable)?,
    })
}
impl Store {
    pub fn start_generation(&self, c: StartGeneration) -> Result<GenerationJob> {
        c.snapshot
            .provider
            .validate()
            .map_err(|e| invalid(&e.to_string()))?;
        ValidatedSettings::try_from(c.snapshot.settings.clone())
            .map_err(|e| invalid(&e.to_string()))?;
        let exposed: BTreeSet<_> = c.snapshot.exposed.iter().collect();
        let control: BTreeSet<_> = c.snapshot.control.iter().collect();
        if exposed.is_empty() {
            return Err(AppError::new(
                ErrorCode::NoTargets,
                "没有可用于生成的目标词",
            ));
        }
        if !exposed.is_disjoint(&control)
            || exposed.len() != c.snapshot.exposed.len()
            || control.len() != c.snapshot.control.len()
        {
            return Err(invalid("目标分组重复或重叠"));
        }
        let request = serde_json::to_string(&c.snapshot).map_err(unavailable)?;
        let revision =
            i64::try_from(c.snapshot.provider.revision).map_err(|_| invalid("配置版本超出范围"))?;
        let value=self.command(&c.profile,&c.operation,"start_generation",&request,c.now,|tx|{
            tx.execute("INSERT INTO jobs(id,profile_id,operation_id,kind,phase,config_revision,status,created_at,updated_at) VALUES (?1,?2,?3,'generation','queued',?4,'queued',?5,?5)",params![c.job,c.profile,c.operation,revision,c.now]).map_err(unavailable)?;
            tx.execute("INSERT INTO generation_snapshots VALUES (?1,?2)",params![c.job,request]).map_err(unavailable)?;
            Ok((json!(c.job),vec![Event{id:c.event,kind:"generation_requested".into(),schema_version:1,payload:json!({"jobId":c.job,"configRevision":revision})}]))
        })?;
        self.generation_job(
            &c.profile,
            value.as_str().ok_or_else(|| unavailable("receipt"))?,
        )
    }
    pub fn generation_job(&self, profile: &str, id: &str) -> Result<GenerationJob> {
        let db = self.connection.lock().map_err(unavailable)?;
        job(&db, profile, id)
    }
    pub fn generation_snapshot(&self, profile: &str, id: &str) -> Result<GenerationSnapshot> {
        let db = self.connection.lock().map_err(unavailable)?;
        let value:Option<String>=db.query_row("SELECT s.snapshot_json FROM generation_snapshots s JOIN jobs j ON j.id=s.job_id WHERE j.profile_id=?1 AND j.id=?2",params![profile,id],|r|r.get(0)).optional().map_err(unavailable)?;
        serde_json::from_str(&value.ok_or(AppError::new(ErrorCode::NotFound, "生成快照不存在"))?)
            .map_err(unavailable)
    }
    /// Claim exactly once before any network call. A duplicate dispatcher must not call a model.
    pub fn claim_generation(&self, profile: &str, id: &str, now: i64) -> Result<bool> {
        let mut db = self.connection.lock().map_err(unavailable)?;
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        let changed=tx.execute("UPDATE jobs SET status='running',phase='generating',updated_at=?3 WHERE profile_id=?1 AND id=?2 AND status='queued'",params![profile,id,now]).map_err(unavailable)?;
        if changed == 1 {
            tx.execute(
                "INSERT INTO events VALUES (?1,?2,NULL,?3,0,'generation_started',1,?4,?5)",
                params![
                    format!("generation-start-{id}"),
                    profile,
                    format!("generation-start-{id}"),
                    json!({"jobId":id}).to_string(),
                    now
                ],
            )
            .map_err(unavailable)?;
        }
        tx.commit().map_err(unavailable)?;
        Ok(changed == 1)
    }
    pub fn stop_generation(
        &self,
        profile: &str,
        id: &str,
        outcome: JobStatus,
        now: i64,
    ) -> Result<GenerationJob> {
        let (state, message) = match outcome {
            JobStatus::Cancelled => ("cancelled", "生成已取消"),
            JobStatus::Failed => ("failed", "生成失败，可检查配置后重新提交"),
            JobStatus::Interrupted => ("interrupted", "生成被中断，不会自动重试"),
            _ => return Err(invalid("非法终止状态")),
        };
        let operation = format!("generation-{state}-{id}");
        self.command(profile,&operation,"stop_generation",&json!({"job":id,"status":state}).to_string(),now,|tx|{
            let previous=job(tx,profile,id)?;
            if matches!(previous.status,JobStatus::Queued|JobStatus::Running){
                tx.execute("UPDATE jobs SET status=?3,phase=?3,error=?4,updated_at=?5 WHERE profile_id=?1 AND id=?2",params![profile,id,state,message,now]).map_err(unavailable)?;
            }
            Ok((json!(id),vec![Event{id:operation.clone(),kind:"generation_stop".into(),schema_version:1,payload:json!({"jobId":id,"requestedStatus":state,"previousStatus":previous.status})}]))
        })?;
        self.generation_job(profile, id)
    }
    pub fn interrupt_generation_jobs(&self, profile: &str, now: i64) -> Result<u64> {
        let ids: Vec<String> = {
            let db = self.connection.lock().map_err(unavailable)?;
            let mut statement=db.prepare("SELECT id FROM jobs WHERE profile_id=?1 AND kind='generation' AND status IN ('queued','running')").map_err(unavailable)?;
            statement
                .query_map([profile], |r| r.get(0))
                .map_err(unavailable)?
                .collect::<std::result::Result<_, _>>()
                .map_err(unavailable)?
        };
        for id in &ids {
            self.stop_generation(profile, id, JobStatus::Interrupted, now)?;
        }
        Ok(ids.len() as u64)
    }
    pub fn save_article(&self, c: SaveArticle) -> Result<Article> {
        if c.title.trim().is_empty() || c.body.trim().is_empty() {
            return Err(invalid("文章标题与正文不能为空"));
        }
        if c.genre == cijian_domain::settings::Genre::Auto {
            return Err(invalid("保存前必须确定文章体裁"));
        }
        let mut targets = BTreeSet::new();
        if c.targets.iter().any(|t| !targets.insert(&t.lexeme_id)) {
            return Err(invalid("文章目标不能重复"));
        }
        let body_hash = hash(&c.body);
        let analysis = json!({"analysis":c.analysis,"diagnostics":c.diagnostics});
        let request=json!({"job":c.job,"article":c.id,"title":c.title,"body":c.body,"genre":c.genre,"tier":c.tier,"analysis":analysis,"targets":c.targets,"model":c.model,"provider":c.provider_fingerprint}).to_string();
        let value=self.command(&c.profile,&c.operation,"save_article",&request,c.now,|tx|{
            if job(tx,&c.profile,&c.job)?.status!=JobStatus::Running{return Err(AppError::new(ErrorCode::Cancelled,"任务已结束，未保存迟到的生成结果"));}
            tx.execute("INSERT INTO articles VALUES (?1,?2,?3,?4,?5,?6,?7,'article.v1',?8,?9,'generated',NULL,?10,'rust-analyzer.v1','experiment.v1',?11)",params![c.id,c.profile,c.title,c.body,body_hash,serde_json::to_string(&c.genre).map_err(unavailable)?,serde_json::to_string(&c.tier).map_err(unavailable)?,c.model,c.provider_fingerprint,analysis.to_string(),c.now]).map_err(unavailable)?;
            for target in &c.targets{tx.execute("INSERT INTO article_targets VALUES (?1,?2,?3,?4,?5,?6)",params![c.profile,c.id,target.lexeme_id,match target.role{TargetRole::Exposed=>"exposed",TargetRole::Control=>"control"},target.occurrences,target.selection_reason]).map_err(unavailable)?;}
            tx.execute("UPDATE jobs SET status='succeeded',phase='saved',result_ref=?3,updated_at=?4 WHERE profile_id=?1 AND id=?2",params![c.profile,c.job,c.id,c.now]).map_err(unavailable)?;
            Ok((json!(c.id),vec![Event{id:c.event,kind:"article_generate".into(),schema_version:1,payload:json!({"articleId":c.id,"jobId":c.job,"bodyHash":body_hash})}]))
        })?;
        self.article(
            &c.profile,
            value.as_str().ok_or_else(|| unavailable("receipt"))?,
        )
    }
    pub fn article(&self, profile: &str, id: &str) -> Result<Article> {
        let db = self.connection.lock().map_err(unavailable)?;
        let value:Option<ArticleRow>=db.query_row("SELECT id,title,body,body_hash,genre,tier,analysis_json,created_at FROM articles WHERE profile_id=?1 AND id=?2",params![profile,id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?))).optional().map_err(unavailable)?;
        let (id, title, body, body_hash, genre, tier, analysis, created_at) =
            value.ok_or(AppError::new(ErrorCode::NotFound, "文章不存在"))?;
        let stored: serde_json::Value = serde_json::from_str(&analysis).map_err(unavailable)?;
        Ok(Article {
            id,
            title,
            body,
            body_hash,
            genre: serde_json::from_str(&genre).map_err(unavailable)?,
            tier: serde_json::from_str(&tier).map_err(unavailable)?,
            analysis: serde_json::from_value(stored["analysis"].clone()).map_err(unavailable)?,
            diagnostics: serde_json::from_value(stored["diagnostics"].clone())
                .map_err(unavailable)?,
            created_at,
        })
    }
    pub fn articles(
        &self,
        profile: &str,
        from: Option<i64>,
        until: Option<i64>,
        page: u32,
    ) -> Result<Vec<Article>> {
        let ids: Vec<String> = {
            let db = self.connection.lock().map_err(unavailable)?;
            let mut statement=db.prepare("SELECT id FROM articles WHERE profile_id=?1 AND (?2 IS NULL OR created_at>=?2) AND (?3 IS NULL OR created_at<?3) ORDER BY created_at DESC,id LIMIT 30 OFFSET ?4").map_err(unavailable)?;
            statement
                .query_map(params![profile, from, until, i64::from(page) * 30], |r| {
                    r.get(0)
                })
                .map_err(unavailable)?
                .collect::<std::result::Result<_, _>>()
                .map_err(unavailable)?
        };
        ids.iter().map(|id| self.article(profile, id)).collect()
    }
}
