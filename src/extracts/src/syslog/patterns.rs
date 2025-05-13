use lazy_static::lazy_static;
use predicates::{prelude::predicate, str::RegexPredicate};

pub struct SyslogRegexPattern {
    pub id: String,
    pub display_name: String,
    pub regex: RegexPredicate,
}

impl SyslogRegexPattern {
    pub fn new(id: String, display_name: String, regex: String) -> SyslogRegexPattern {
        SyslogRegexPattern {
            id,
            display_name,
            regex: predicate::str::is_match(regex).unwrap(),
        }
    }
}

lazy_static! {
    pub static ref SYSLOG_PATTERNS: Vec<SyslogRegexPattern> = vec![SyslogRegexPattern::new(
        "OUT_OF_MEMORY".to_string(),
        "Out of memory".to_string(),
        "(?i)Out of memory".to_string()
    )];
}
