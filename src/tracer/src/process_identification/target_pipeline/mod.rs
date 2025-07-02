//! This module contains the machinery for identifying pipline steps from target processes.
//!
//! There are three possible execution scenarios to consider:
//! 1. Processes running within containers
//! 2. Processes not running in containers
//!    a. Distributed workflow in which all processes for a task are executed in a single VM
//!    b. Local workflow in which all processes run in the same VM
//!
//! Linking processes to steps is essentially the same for (1) and (2a); for (2b) we can
//! potentially link a process to a step name if the process is only used in a single step,
//! but if the same step is executed multiple times we can't link a particular process to a
//! particular step instance. There are also hybrid scenarios, e.g. a local workflow that
//! uses containers only for some steps.
//!
//! The linkage between processes -> step is based on step signatures, where a signature is a
//! set of processes that are expected to be matched (i.e., recognized by a target rule and
//! actively monitored) and an optional set of additional processes started but not matched.
//! A score is computed for each process set for each step, and if the score is above a threshold,
//! the step matches. If the pipeline ID is known ahead of time, it can be used to narrow the set
//! of steps to consider.
mod parser;
pub mod pipeline_manager;
