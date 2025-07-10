/// EC2MatchEngine is responsible for selecting the best-priced EC2 instance offering
/// from the AWS Pricing API results based on a set of weighted match criteria.
///
/// # Purpose
/// AWS Pricing API often returns multiple pricing entries for a given instance type,
/// differing by OS, tenancy, license model, etc. Without filtering, it's easy to
/// mistakenly pick an irrelevant or high-cost option (e.g., SQL Server or Dedicated tenancy).
///
/// # How it works
/// - Each candidate `PricingData` entry is scored against the real instance metadata
///   (`FilterableInstanceDetails`) using predefined match fields.
/// - Match fields include: region, instance type, tenancy, and operating system.
/// - Each field has a weighted score (e.g., region and instance type are weighted higher).
/// - The engine filters out incompatible entries, ranks remaining candidates by total score,
///   and sorts by price if scores are equal.
/// - The top N matches are returned, allowing selection of the best match and a fallback.
///
/// # Why it's necessary
/// This approach ensures:
/// - Only relevant pricing entries are considered.
/// - Overpriced or incorrect matches (e.g., Windows or SQL pricing) are excluded.
/// - The returned pricing is both accurate and stable across regions and configurations.
/// - Future extensions (e.g., adding capacity status or license model) are easy to support
///   by expanding the match field list.
///
/// This engine is critical to avoid false cost estimates and enables confidence in
/// Tracer’s EC2 cost tracking and optimization features.
///
use crate::cloud_providers::aws::types::pricing::{
    FilterableInstanceDetails, FlattenedData, PricingData,
};

#[derive(Debug, Clone)]
enum MatchField {
    Region,
    InstanceType,
    Tenancy,
    OperatingSystem,
    //CapacityStatus,
}

impl MatchField {
    fn score(&self) -> u8 {
        match self {
            MatchField::Region => 10,
            MatchField::InstanceType => 10,
            MatchField::Tenancy => 5,
            MatchField::OperatingSystem => 5,
        }
    }
}

pub struct EC2MatchEngine {
    target: FilterableInstanceDetails,
    candidates: Vec<PricingData>,
}

impl EC2MatchEngine {
    pub fn new(target: FilterableInstanceDetails, candidates: Vec<PricingData>) -> Self {
        Self { target, candidates }
    }
    pub fn best_matches(&self, top_n: usize) -> Vec<FlattenedData> {
        let max_score = self.max_score();
        let mut scored: Vec<(u8, &PricingData)> = self
            .candidates
            .iter()
            .filter(|p| self.matches(p))
            .map(|p| (self.score(p), p))
            .collect();

        // Sort by score descending, fallback to price ascending
        scored.sort_by(|(score_a, a), (score_b, b)| {
            score_b.cmp(score_a).then_with(|| {
                let price_a = FlattenedData::flatten_data(a).price_per_unit;
                let price_b = FlattenedData::flatten_data(b).price_per_unit;
                price_a
                    .partial_cmp(&price_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });

        scored
            .into_iter()
            .take(top_n)
            .map(|(score, p)| {
                let mut flattened = FlattenedData::flatten_data(p);
                flattened.match_percentage = Some((score as f64 / max_score as f64) * 100.0);
                flattened
            })
            .collect()
    }

    fn field_matches(&self, field: MatchField, p: &PricingData) -> bool {
        match field {
            MatchField::Region => p.region_code == self.target.region,
            MatchField::InstanceType => p.instance_type == self.target.instance_type,
            MatchField::Tenancy => self
                .target
                .tenancy
                .as_ref()
                .zip(p.tenancy.as_ref())
                .is_some_and(|(target, candidate)| target.eq_ignore_ascii_case(candidate)),
            MatchField::OperatingSystem => self
                .target
                .operating_system
                .as_ref()
                .zip(p.operating_system.as_ref())
                .is_some_and(|(target, candidate)| {
                    candidate.to_lowercase().contains(&target.to_lowercase())
                }),
        }
    }

    fn score(&self, p: &PricingData) -> u8 {
        [
            MatchField::Region,
            MatchField::InstanceType,
            MatchField::Tenancy,
            MatchField::OperatingSystem,
        ]
        .into_iter()
        .filter(|field| self.field_matches(field.clone(), p))
        .map(|field| field.score())
        .sum()
    }

    fn matches(&self, p: &PricingData) -> bool {
        [
            MatchField::Region,
            MatchField::InstanceType,
            MatchField::Tenancy,
            MatchField::OperatingSystem,
        ]
        .iter()
        .all(|field| self.field_matches(field.clone(), p))
    }

    fn max_score(&self) -> u8 {
        [
            MatchField::Region,
            MatchField::InstanceType,
            MatchField::Tenancy,
            MatchField::OperatingSystem,
        ]
        .iter()
        .map(|f| f.score())
        .sum()
    }
}
