use std::{
    fs,
    path::{Path, PathBuf},
    process,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use ataridisk::{config::Config, error, layout::DiskLayout, storage::DiskStorage};
use serialport::{ClearBuffer, DataBits, FlowControl, Parity, SerialPort, StopBits};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opt {
    /// List available ports and close the app
    #[structopt(long)]
    list_availables: bool,

    /// Config file to load
    #[structopt(long, short, default_value = "config.json")]
    config_path: PathBuf,

    /// Port to connect with
    #[structopt(long, short, default_value = "/dev/ttyUSB0")]
    port: String,

    /// Path to import as virtual disk content
    load_path: PathBuf,
}

impl Opt {
    fn config(&self) -> Config {
        fn load_config(path: &Path) -> Option<Config> {
            let content = fs::read_to_string(path).ok()?;
            serde_json::from_str(&content).ok()
        }

        load_config(&self.config_path).unwrap_or_default()
    }
}

/// Print available ports on screen then exit.
fn print_availables() -> error::Result<()> {
    println!("Available ports:");
    for port in serialport::available_ports()? {
        println!("- {}", port.port_name);
    }

    Ok(())
}

fn wait_sigterm() -> anyhow::Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })?;

    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(250));
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let opt = Opt::from_args();

    if opt.list_availables {
        print_availables()?;
        return Ok(());
    }

    let config = opt.config();
    log::info!("Configuration: {:?}", config);

    let mut serial = serialport::new(&opt.port, 19200)
        .parity(Parity::None)
        .timeout(Duration::from_secs(24 * 3600))
        .flow_control(FlowControl::None)
        .data_bits(DataBits::Eight)
        .stop_bits(StopBits::One)
        .open_native()?;

    serial.clear(ClearBuffer::All)?;

    let disk_layout = DiskLayout::new(
        config.tos.clone(),
        config.partition_type.clone(),
        config.root_directory_sectors(),
    );
    let mut storage = DiskStorage::new(disk_layout);

    let t_start = Instant::now();
    storage.import_path(&opt.load_path)?;
    let t_load = t_start.elapsed();

    log::info!("Ready in {:}ms", t_load.as_millis());

    println!("Atari serial disk: READY.");
    println!("Press ^C to exit.");

    // Start listener thread
    thread::Builder::new()
        .name("listener".to_string())
        .spawn(move || {
            if let Err(error) = ataridisk::state_machine::run(&mut storage, &mut serial) {
                log::error!("Listener thread as crash. Closing app (error: {})", error);
                process::exit(1);
            }
        })?;

    // Wait for stop signal
    wait_sigterm()?;

    log::info!("All done. Bye !");
    Ok(())
}
