use anyhow::Result;
use dashmap::DashMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::{self, Ordering};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::LazyLock;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;
use tracing::warn;

static REGEX_CACHE: LazyLock<Arc<DashMap<String, Regex>>> =
    LazyLock::new(|| Arc::new(DashMap::new()));

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CachedRegex(String);

impl CachedRegex {
    pub fn new(regex_str: String) -> Result<Self> {
        if !REGEX_CACHE.contains_key(&regex_str) {
            let regex = Regex::new(&regex_str)?;
            REGEX_CACHE.insert(regex_str.clone(), regex);
        };
        Ok(Self(regex_str))
    }

    pub fn is_match(&self, text: &str) -> bool {
        REGEX_CACHE.get(&self.0).unwrap().is_match(text)
    }
}

/// Simple target matching conditions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MatchType {
    ProcessNameIs(String),
    ProcessNameContains(String),
    MinArgs(usize),
    ArgsContain(String),
    ArgsNotContain(String),
    FirstArgIs(String),
    CommandContains(String),
    CommandNotContains(String),
    CommandMatchesRegex(CachedRegex),
    SubcommandIsOneOf(SubcommandSet),
    JavaCommand(String),
    JavaCommandIsOneOf {
        jar: String,
        commands: SubcommandSet,
    },
    And(Vec<MatchType>),
    Or(Vec<MatchType>),
}

impl MatchType {
    pub fn matches(&self, process: &ProcessStartTrigger) -> bool {
        self.get_match(process).is_some()
    }

    /// Returns a ProcessMatch if `process` matches this MatchType, otherwise None
    pub fn get_match<'a>(&self, process: &'a ProcessStartTrigger) -> Option<ProcessMatch<'a>> {
        match self {
            MatchType::ProcessNameIs(name) if process.comm == *name => Some(ProcessMatch::Simple),
            MatchType::ProcessNameContains(substr) if process.comm.contains(substr) => {
                Some(ProcessMatch::Simple)
            }
            MatchType::MinArgs(n) if process.argv.len() > *n => Some(ProcessMatch::Simple),
            MatchType::ArgsContain(content)
                if process.argv.iter().skip(1).any(|arg| arg == content) =>
            {
                Some(ProcessMatch::Simple)
            }
            MatchType::ArgsNotContain(content)
                if !process.argv.iter().skip(1).any(|arg| arg == content) =>
            {
                Some(ProcessMatch::Simple)
            }
            MatchType::FirstArgIs(arg) if process.argv.get(1) == Some(arg) => {
                Some(ProcessMatch::Simple)
            }
            MatchType::CommandContains(content) if process.command_string.contains(content) => {
                Some(ProcessMatch::Simple)
            }
            MatchType::CommandNotContains(content) if !process.command_string.contains(content) => {
                Some(ProcessMatch::Simple)
            }
            MatchType::CommandMatchesRegex(regex) if regex.is_match(&process.command_string) => {
                Some(ProcessMatch::Simple)
            }
            MatchType::SubcommandIsOneOf(subcommands) => {
                // to find the subcommand, we find the first argument that doesn't start with '-'
                // (as options are usually done with -)
                process
                    .argv
                    .iter()
                    .skip(1)
                    .filter(|arg| !arg.starts_with('-'))
                    .find(|arg| subcommands.contains(arg))
                    .map(|cmd| ProcessMatch::Subcommand(cmd))
            }
            MatchType::JavaCommand(jar) => {
                match_java(process, jar, None).map(ProcessMatch::Subcommand)
            }
            MatchType::JavaCommandIsOneOf { jar, commands } => {
                match_java(process, jar, Some(commands)).map(ProcessMatch::Subcommand)
            }
            MatchType::And(conditions) => {
                // saving the subcommand in case in the AND condition a subcommand is found
                conditions
                    .iter()
                    .map(|condition| condition.get_match(process))
                    .reduce(|a, b| match (a, b) {
                        (None, _) | (_, None) => None,
                        (Some(a), Some(b)) => Some(cmp::max(a, b)),
                    })
                    .flatten()
            }
            MatchType::Or(conditions) => conditions
                .iter()
                .filter_map(|condition| condition.get_match(process))
                .next(),

            _ => None,
        }
    }
}

fn match_java<'a>(
    process: &'a ProcessStartTrigger,
    jar: &str,
    subcommands: Option<&SubcommandSet>,
) -> Option<&'a str> {
    if process.comm.contains("java") {
        let mut args = process.argv.iter().skip(1);
        if args
            .find(|arg| *arg == "-jar")
            .and_then(|_| args.next())
            .map(|arg| arg.contains(jar))
            .unwrap_or(false)
        {
            for arg in args {
                if arg.starts_with('-') {
                    continue;
                }
                if let Some(subcommands) = subcommands {
                    if subcommands.contains(arg) {
                        return Some(arg);
                    }
                } else {
                    return Some(arg);
                }
            }
        }
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubcommandSet(HashSet<String>);

impl SubcommandSet {
    pub fn contains(&self, item: &str) -> bool {
        self.0.contains(item)
    }
}

impl<I: IntoIterator<Item = String>> From<I> for SubcommandSet {
    fn from(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl Hash for SubcommandSet {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for item in self.0.iter() {
            item.hash(state);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProcessMatch<'a> {
    Simple,
    Subcommand(&'a str),
}

impl<'a> ProcessMatch<'a> {
    pub fn with_subcommand(sub_command: &'a str) -> Self {
        Self::Subcommand(sub_command)
    }
}

impl Ord for ProcessMatch<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (ProcessMatch::Subcommand(sub_command1), ProcessMatch::Subcommand(sub_command2))
                if sub_command1 == sub_command2 =>
            {
                Ordering::Equal
            }
            (ProcessMatch::Subcommand(sub_command1), ProcessMatch::Subcommand(sub_command2)) => {
                warn!(
                    "Matched two different subcommands: {} and {}",
                    sub_command1, sub_command2
                );
                Ordering::Less
            }
            (ProcessMatch::Subcommand { .. }, _) => Ordering::Greater,
            (_, ProcessMatch::Subcommand { .. }) => Ordering::Less,
            _ => Ordering::Equal,
        }
    }
}

impl PartialOrd for ProcessMatch<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
