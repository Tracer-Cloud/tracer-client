#![no_std]
#![no_main]

use aya_ebpf::bindings::path;
use aya_ebpf::helpers::gen::bpf_probe_read_kernel_str;
use aya_ebpf::helpers::{
    bpf_d_path, bpf_probe_read, bpf_probe_read_kernel, bpf_probe_read_kernel_buf,
    bpf_probe_read_user, bpf_probe_read_user_str_bytes,
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
use tracer_ebpf_common::process_enter::{ProcessEnterType, ProcessRawTrigger, MAX_NUM_ARGS};
use tracer_ebpf_kernel::gen::{file, linux_binprm, mm_struct, task_struct};

#[map]
static mut EVENTS: PerfEventArray<ProcessRawTrigger> = PerfEventArray::new(0);

#[map]
static mut BUFFER: PerCpuArray<ProcessRawTrigger> = PerCpuArray::with_max_entries(1, 0);

#[btf_tracepoint(function = "sched_process_exec")]
pub fn sched_process_exec(ctx: BtfTracePointContext) -> i64 {
    unsafe { try_sched_process_exec(ctx).unwrap_or_else(|ret| ret) }
}

#[btf_tracepoint(function = "sched_process_exit")]
pub fn sched_process_exit(ctx: BtfTracePointContext) -> i64 {
    unsafe { try_sched_process_exit(ctx).unwrap_or_else(|ret| ret) }
}

unsafe fn try_sched_process_exec(ctx: BtfTracePointContext) -> Result<i64, i64> {
    info!(&ctx, "tracepoint sched_process_exec called");

    let task: *const task_struct = unsafe { ctx.arg(0) };

    if task.is_null() {
        return Err(-1);
    }

    let Some(mut event) = BUFFER.get_ptr_mut(0) else {
        return Err(-1);
    };

    let event = &mut *event;
    event.pid = (*task).pid;
    event.event_type = ProcessEnterType::Start;

    event.time = (*task).start_time as u64;

    let parent_task_struct = (*task).real_parent as *const task_struct;

    if !parent_task_struct.is_null() {
        event.ppid = (*parent_task_struct).pid;
    }

    let mm = bpf_probe_read::<*mut mm_struct>((*task).mm as *const *mut _)?;
    if mm.is_null() {
        return Err(-1);
    }

    let mm: *mut mm_struct = bpf_probe_read_kernel(&(*task).mm)?;
    event.comm = (*task).comm;

    let linux_binprm: *const linux_binprm = ctx.arg(2);

    if linux_binprm.is_null() {
        return Err(-1);
    }

    let filename = (*linux_binprm).filename;

    let filename = bpf_probe_read_kernel_str_bytes(filename as *const u8, &mut event.file_name)?;

    let mut arg_start = bpf_probe_read_kernel(&(*mm).__bindgen_anon_1.arg_start)?;
    let arg_end = bpf_probe_read_kernel(&(*mm).__bindgen_anon_1.arg_end)?;

    let args_len = (arg_end - arg_start) as u32;
    event.len = 0;

    for i in 0..MAX_NUM_ARGS {
        if arg_start >= arg_end {
            break;
        }

        let arg = bpf_probe_read_user_str_bytes(arg_start as *const u8, &mut event.argv[i])?;
        event.len += 1;

        if arg.is_empty() {
            break;
        }

        let l = arg.len();
        arg_start += l as u64 + 1; // +1 for null terminator
    }

    EVENTS.output(&ctx, &event, 0);
    info!(&ctx, "sched_process_exec: sent data");

    Ok(0)
}

unsafe fn try_sched_process_exit(ctx: BtfTracePointContext) -> Result<i64, i64> {
    info!(&ctx, "tracepoint sched_process_exit called");

    let task: *const task_struct = unsafe { ctx.arg(0) };

    if task.is_null() {
        return Err(-1);
    }

    if (*task).pid != (*task).tgid {
        return Ok(0);
    }

    let Some(mut event) = BUFFER.get_ptr_mut(0) else {
        return Err(-1);
    };

    let event = &mut *event;
    event.pid = (*task).pid;
    event.event_type = ProcessEnterType::Finish;

    EVENTS.output(&ctx, &event, 0);
    info!(&ctx, "sched_process_exit: sent data");

    Ok(0)
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
