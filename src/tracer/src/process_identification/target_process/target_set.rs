use crate::process_identification::target_process::target::Target;
use crate::process_identification::target_process::target_match::MatchType;
use std::collections::{HashMap, HashSet};
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

#[derive(Debug, Clone)]
pub struct TargetSet {
    process_name_is: HashMap<String, Target>,
    other: HashSet<Target>,
}

impl TargetSet {
    pub fn new<I: IntoIterator<Item = Target>>(targets: I) -> Self {
        let (process_name_is, other) = targets.into_iter().fold(
            (HashMap::new(), HashSet::new()),
            |(mut process_name_is, mut other), mut target| {
                match target.match_type_mut() {
                    MatchType::Or(match_types) => {
                        for i in (0..match_types.len()).rev() {
                            if let MatchType::ProcessNameIs(process_name) = &match_types[i] {
                                process_name_is.insert(
                                    process_name.clone(),
                                    Target::new(match_types.remove(i)),
                                );
                            }
                        }
                        if !match_types.is_empty() {
                            other.insert(Target::new(MatchType::Or(match_types.clone())));
                        }
                    }
                    MatchType::ProcessNameIs(process_name) => {
                        process_name_is.insert(process_name.clone(), target);
                    }
                    _ => {
                        other.insert(target);
                    }
                };
                (process_name_is, other)
            },
        );
        Self {
            process_name_is,
            other,
        }
    }

    pub fn matches(&self, process: &ProcessStartTrigger) -> bool {
        self.process_name_is.contains_key(&process.comm)
            || self.other.iter().any(|target| target.matches(process))
    }

    pub fn get_match(&self, process: &ProcessStartTrigger) -> Option<String> {
        self.process_name_is
            .get(&process.comm)
            .map(|target| target.display_name().to_string())
            .or_else(|| {
                self.other
                    .iter()
                    .find_map(|target| target.get_match(process))
            })
    }
}

impl<I: IntoIterator<Item = Target>> From<I> for TargetSet {
    fn from(iter: I) -> Self {
        Self::new(iter)
    }
}
