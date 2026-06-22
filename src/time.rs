use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DateFilter {
    pub unix: i64,
    pub date: String,
}

pub fn parse_date_filter(field: &'static str, value: &str) -> Result<DateFilter> {
    let trimmed = value.trim();
    let unix = parse_timestamp(trimmed).map_err(|message| Error::InvalidDate {
        field,
        value: value.to_owned(),
        message,
    })?;
    let date = DateTime::from_timestamp(unix, 0)
        .ok_or_else(|| Error::InvalidDate {
            field,
            value: value.to_owned(),
            message: "timestamp is outside the supported range".to_owned(),
        })?
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();

    Ok(DateFilter { unix, date })
}

fn parse_timestamp(value: &str) -> std::result::Result<i64, String> {
    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        let datetime = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| "date could not be represented at midnight".to_owned())?;
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc).timestamp());
    }

    let normalized = normalize_short_offset(value);
    for format in ["%Y-%m-%d %H:%M:%S%:z", "%Y-%m-%dT%H:%M:%S%:z"] {
        if let Ok(datetime) = DateTime::parse_from_str(&normalized, format) {
            return Ok(datetime.timestamp());
        }
    }

    for format in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(datetime) = NaiveDateTime::parse_from_str(value, format) {
            return Ok(DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc).timestamp());
        }
    }

    Err("expected YYYY-MM-DD, YYYY-MM-DD HH:MM:SS±HH[:MM], or RFC3339-like datetime".to_owned())
}

fn normalize_short_offset(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() < 3 {
        return value.to_owned();
    }

    let sign_index = bytes.len() - 3;
    let sign = bytes[sign_index];
    let hour_1 = bytes[sign_index + 1];
    let hour_2 = bytes[sign_index + 2];

    if matches!(sign, b'+' | b'-') && hour_1.is_ascii_digit() && hour_2.is_ascii_digit() {
        format!("{value}:00")
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::parse_date_filter;

    #[test]
    fn parses_date_only_as_utc_midnight() {
        let parsed = parse_date_filter("before", "2026-05-01").unwrap();

        assert_eq!(parsed.unix, 1_777_593_600);
        assert_eq!(parsed.date, "2026-05-01");
    }

    #[test]
    fn parses_datetime_with_short_numeric_offset() {
        let parsed = parse_date_filter("before", "2026-05-01 12:31:01-06").unwrap();

        assert_eq!(parsed.unix, 1_777_660_261);
        assert_eq!(parsed.date, "2026-05-01");
    }

    #[test]
    fn rejects_invalid_date_filter() {
        let error = parse_date_filter("after", "last Tuesday").unwrap_err();

        assert!(error.to_string().contains("invalid after value"));
    }
}
