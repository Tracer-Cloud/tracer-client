use aya::{programs::BtfTracePoint, Btf, Ebpf};
#[rustfmt::skip]
use log::{debug, warn};


pub struct TracerEbpf {
    ebpf: Ebpf,
}

impl TracerEbpf {
    pub fn new(ebpf: aya::Ebpf) -> Self {
        Self { ebpf }
    }

    pub fn ebpf(&self) -> &aya::Ebpf {
        &self.ebpf
    }
}



pub fn load_ebpf() -> anyhow::Result<TracerEbpf> {
    env_logger::init();

    // Bump the memlock rlimit. This is needed for older kernels that don't use the
    // new memcg based accounting, see https://lwn.net/Articles/837122/
    let rlim = libc::rlimit {
        rlim_cur: libc::RLIM_INFINITY,
        rlim_max: libc::RLIM_INFINITY,
    };
    let ret = unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlim) };
    if ret != 0 {
        debug!("remove limit on locked memory failed, ret is: {}", ret);
    }

    // This will include your eBPF object file as raw bytes at compile-time and load it at
    // runtime. This approach is recommended for most real-world use cases. If you would
    // like to specify the eBPF program at runtime rather than at compile-time, you can
    // reach for `Bpf::load_file` instead.
    let mut ebpf = aya::Ebpf::load(aya::include_bytes_aligned!(concat!(
        env!("OUT_DIR"),
        "/tracer_ebpf"
    )))?;
    if let Err(e) = aya_log::EbpfLogger::init(&mut ebpf) {
        // This can happen if you remove all log statements from your eBPF program.
        warn!("failed to initialize eBPF logger: {}", e);
    }

    let btf = Btf::from_sys_fs()?;

    // todo: has_attach_point && is_compatible

    let program: &mut BtfTracePoint = ebpf.program_mut("sched_process_exec").unwrap().try_into()?;
    program.load("sched_process_exec", &btf)?;
    program.attach()?;

    Ok(TracerEbpf::new(ebpf))
}
