# AWS Pricing Module - Functional Architecture

This module has been refactored from a monolithic structure to a functional, composable architecture with clear separation of concerns.

## Module Structure

```
pricing/
├── client.rs              # Main pricing client (entry point)
├── context_builder.rs     # Pricing context composition
├── ec2_client_manager.rs  # EC2 client lifecycle management
├── ec2_pricing.rs         # EC2 pricing data fetching
├── ebs_pricing.rs         # EBS pricing calculations
├── filter_builder.rs     # AWS pricing filter construction
├── filtering/             # Pricing matching engines
├── tests.rs              # Comprehensive test suite
└── mod.rs                # Module exports and PricingSource enum
```

## Functional Design Principles

### 1. **Pure Functions**
- Functions have no side effects where possible
- Predictable inputs and outputs
- Easy to test and reason about

### 2. **Composition Over Inheritance**
- Small, focused functions that compose together
- Pipeline-style data processing
- Functional error handling with `Option` and `Result`

### 3. **Separation of Concerns**
- Each module has a single responsibility
- Clear boundaries between different pricing concerns
- Minimal coupling between modules

## Key Functions

### Client Management (`client.rs`)
```rust
pub async fn get_instance_pricing_context_from_metadata(
    &self,
    metadata: &AwsInstanceMetaData,
) -> Option<InstancePricingContext>
```
Main entry point that orchestrates the pricing pipeline.

### Context Building (`context_builder.rs`)
```rust
pub async fn build_pricing_context(
    pricing_client: &pricing::Client,
    ec2_client: &Ec2Client,
    metadata: &AwsInstanceMetaData,
) -> Option<InstancePricingContext>
```
Functional pipeline: describe → filter → fetch → match → combine

### EC2 Pricing (`ec2_pricing.rs`)
```rust
pub async fn fetch_ec2_pricing_data(
    pricing_client: &pricing::Client,
    filters: Vec<PricingFilters>,
) -> Option<Vec<PricingData>>
```
Pure function for fetching EC2 pricing with retry logic.

### EBS Pricing (`ebs_pricing.rs`)
```rust
pub async fn calculate_total_ebs_cost(
    pricing_client: &pricing::Client,
    ec2_client: &Ec2Client,
    region: &str,
    instance_id: &str,
) -> f64
```
Functional composition for EBS cost calculation with free tier handling.

### Filter Building (`filter_builder.rs`)
```rust
pub fn build_ec2_filters(details: &FilterableInstanceDetails) -> Vec<PricingFilters>
pub fn build_ebs_filters(region: &str, volume_type: &str) -> Vec<PricingFilters>
```
Pure functions for constructing AWS pricing API filters.

## Benefits of Functional Architecture

### 1. **Testability**
- Each function can be tested in isolation
- No hidden dependencies or side effects
- Easy to mock and stub dependencies

### 2. **Maintainability**
- Small, focused modules are easier to understand
- Changes are localized to specific concerns
- Clear data flow through the system

### 3. **Reusability**
- Functions can be composed in different ways
- Easy to extract and reuse pricing logic
- Modular design supports different use cases

### 4. **Error Handling**
- Functional error handling with `Option` and `Result`
- Explicit error propagation
- No hidden exceptions or panics

## Migration from Monolithic Structure

The original 500+ line `aws.rs` file has been split into:
- **7 focused modules** (average ~50-100 lines each)
- **Clear functional boundaries**
- **Composable pipeline architecture**
- **Comprehensive test coverage**

## Usage Example

```rust
use crate::cloud_providers::aws::pricing::PricingSource;

// Create pricing source
let pricing_source = PricingSource::new(aws_config).await;

// Get pricing context (functional pipeline)
let context = pricing_source
    .get_aws_price_for_instance(&metadata)
    .await?;

// Access composed pricing data
println!("Total cost: ${:.4}/hr", context.total_hourly_cost);
```

The functional architecture makes the pricing system more maintainable, testable, and easier to extend with new pricing sources or calculation methods.
