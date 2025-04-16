use anyhow::Context;
use aya::maps::perf::AsyncPerfEventArrayBuffer;
use aya::maps::{AsyncPerfEventArray, MapData};
use aya::util::online_cpus;
use aya::{programs::BtfTracePoint, Btf, Ebpf};
#[rustfmt::skip]
use tracing::{debug, warn, info};
use tokio::sync::mpsc::Sender;
use tokio_util::bytes;
use tracer_common::trigger::Trigger;
use tracer_ebpf_common::process_enter::ProcessEnter;

async fn read_event_loop(
    mut buf: AsyncPerfEventArrayBuffer<MapData>,
    tx: Sender<Trigger>,
) -> anyhow::Result<()> {
    let mut data = (0..30)
        .map(|_| bytes::BytesMut::with_capacity(size_of::<ProcessEnter>()))
        .collect::<Vec<_>>();
    loop {
        let events = buf.read_events(&mut data).await?;
        info!("read {} events=", events.read);

        for event in &data[..events.read] {
            let raw_event = unsafe { &*(event.as_ptr() as *const ProcessEnter) };
            info!("raw_event: {:?}", raw_event);
            tx.send(Trigger::New).await?;
        }

        if events.lost > 0 {
            warn!("lost {} events", events.lost);
        }
    }
}

pub async fn process_events(tx: Sender<Trigger>) -> anyhow::Result<()> {
    let mut ebpf = load_ebpf()?;

    let mut events =
        AsyncPerfEventArray::try_from(ebpf.take_map("EVENTS").context("Can't open EVENTS map")?)?;

    for cpu_id in online_cpus().unwrap() {
        let tx = tx.clone();
        let buf = events.open(cpu_id, None)?;
        tokio::spawn(async move { read_event_loop(buf, tx).await });
    }

    Ok(())
}

pub fn load_ebpf() -> anyhow::Result<Ebpf> {
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

    Ok(ebpf)
}
