use crate::ValidationError;
use chrono::{DateTime, Datelike, Days, Duration, LocalResult, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StudyDay {
    pub date: NaiveDate,
    pub start_ms: i64,
    pub end_ms: i64,
}

/// Resolves gaps forward to the first valid local second, overlaps to the earlier instant.
fn boundary(date: NaiveDate, hour: u32, zone: Tz) -> Result<i64, ValidationError> {
    let mut local = date
        .and_hms_opt(hour, 0, 0)
        .ok_or(ValidationError::new("dayStartHour", "无效日界"))?;
    for _ in 0..=172800 {
        match zone.from_local_datetime(&local) {
            LocalResult::Single(value) => return Ok(value.timestamp_millis()),
            LocalResult::Ambiguous(a, b) => return Ok(a.min(b).timestamp_millis()),
            LocalResult::None => {
                local = local
                    .checked_add_signed(Duration::seconds(1))
                    .ok_or(ValidationError::new("date", "时间超出范围"))?
            }
        }
    }
    Err(ValidationError::new("timezone", "无法解析时区边界"))
}

pub fn study_day(now_ms: i64, zone: Tz, hour: u32) -> Result<StudyDay, ValidationError> {
    let now = DateTime::<Utc>::from_timestamp_millis(now_ms)
        .ok_or(ValidationError::new("now", "无效时间"))?;
    let mut date = now.with_timezone(&zone).date_naive();
    if now_ms < boundary(date, hour, zone)? {
        date = date
            .pred_opt()
            .ok_or(ValidationError::new("date", "时间超出范围"))?;
    }
    let next = date
        .succ_opt()
        .ok_or(ValidationError::new("date", "时间超出范围"))?;
    Ok(StudyDay {
        date,
        start_ms: boundary(date, hour, zone)?,
        end_ms: boundary(next, hour, zone)?,
    })
}

pub fn study_week_start(now_ms: i64, zone: Tz, hour: u32) -> Result<i64, ValidationError> {
    let day = study_day(now_ms, zone, hour)?;
    let monday = day
        .date
        .checked_sub_days(Days::new(u64::from(
            day.date.weekday().num_days_from_monday(),
        )))
        .ok_or(ValidationError::new("date", "时间超出范围"))?;
    boundary(monday, hour, zone)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ms(s: &str) -> i64 {
        DateTime::parse_from_rfc3339(s).unwrap().timestamp_millis()
    }
    #[test]
    fn shanghai_four_am_and_year_boundary() {
        let zone = chrono_tz::Asia::Shanghai;
        let before = study_day(ms("2026-01-01T03:59:59+08:00"), zone, 4).unwrap();
        assert_eq!(before.date.to_string(), "2025-12-31");
        let after = study_day(ms("2026-01-01T04:00:00+08:00"), zone, 4).unwrap();
        assert_eq!(after.date.to_string(), "2026-01-01");
        assert_eq!(before.end_ms, after.start_ms);
        assert_eq!(
            study_week_start(after.start_ms, zone, 4).unwrap(),
            ms("2025-12-29T04:00:00+08:00")
        );
    }
    #[test]
    fn daylight_saving_gap_and_overlap() {
        let zone = chrono_tz::America::New_York;
        assert_eq!(
            boundary(NaiveDate::from_ymd_opt(2026, 3, 8).unwrap(), 2, zone).unwrap(),
            ms("2026-03-08T03:00:00-04:00")
        );
        assert_eq!(
            boundary(NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(), 1, zone).unwrap(),
            ms("2026-11-01T01:00:00-04:00")
        );
        let spring = study_day(ms("2026-03-08T12:00:00Z"), zone, 0).unwrap();
        assert_eq!(spring.end_ms - spring.start_ms, 23 * 3600000);
        let autumn = study_day(ms("2026-11-01T12:00:00Z"), zone, 0).unwrap();
        assert_eq!(autumn.end_ms - autumn.start_ms, 25 * 3600000);
    }
}
