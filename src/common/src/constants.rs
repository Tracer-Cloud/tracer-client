pub const WORKING_DIR: &str = "/tmp/tracer/";
pub const PID_FILE: &str = "/tmp/tracer/tracerd.pid";
pub const STDOUT_FILE: &str = "/tmp/tracer/tracerd.out";
pub const STDERR_FILE: &str = "/tmp/tracer/tracerd.err";
pub const LOG_FILE: &str = "/tmp/tracer/daemon.log";
pub const FILE_CACHE_DIR: &str = "/tmp/tracer/tracerd_cache";
pub const DEBUG_LOG: &str = "/tmp/tracer/debug.log";

pub const SYSLOG_FILE: &str = "/var/log/syslog";

pub const REPO_OWNER: &str = "tracer-cloud";
pub const REPO_NAME: &str = "tracer-client";

// TODO: remove dependency from Service url completely
pub const DEFAULT_SERVICE_URL: &str = "https://app.tracer.bio/api";
// todo: move to config^?
