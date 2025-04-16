#![no_std]
#![no_main]

use aya_ebpf::helpers::{
    bpf_probe_read, bpf_probe_read_kernel, bpf_probe_read_user, bpf_probe_read_user_str_bytes,
};
use aya_ebpf::maps::PerCpuArray;
use aya_ebpf::{
    cty::c_char,
    helpers::{bpf_probe_read_kernel_str_bytes, r#gen::bpf_get_current_task_btf},
    macros::{btf_tracepoint, map},
    maps::{PerfEventArray, RingBuf},
    programs::BtfTracePointContext,
};
use aya_log_ebpf::info;

#[btf_tracepoint(function = "sched_process_exec")]
pub fn sched_process_exec(ctx: BtfTracePointContext) -> i64 {
    match try_sched_process_exec(ctx) {
        Ok(ret) => ret,
        Err(ret) => ret,
    }
}

fn try_sched_process_exec(ctx: BtfTracePointContext) -> Result<i64, i64> {
    info!(&ctx, "tracepoint sched_process_exec called");

    Ok(0)
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
