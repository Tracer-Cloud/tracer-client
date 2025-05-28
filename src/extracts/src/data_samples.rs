//use once_cell::sync::Lazy;

// Have this as a seperate Vec because the assumption is the datasamples because it's faster to go
// through a dedicated list than filtering the main target_list to find all CommandContainsV2
// variants
//pub static DATA_SAMPLES_EXT: Lazy<Vec<&'static str>> = Lazy::new(|| vec![".fa", ".fastq"]); //TODO put it back, commented out for clippy
