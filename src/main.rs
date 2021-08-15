mod checksum;
mod config;
mod dos;
mod entries;
mod error;
mod fat;
mod layout;
mod state_machine;
mod storage;

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use config::Config;
use layout::DiskLayout;
use serialport::{ClearBuffer, DataBits, FlowControl, Parity, SerialPort, StopBits};
use storage::DiskStorage;
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
    let mut storage = DiskStorage::new(&disk_layout);

    let t_start = Instant::now();
    storage.import_path(&opt.load_path)?;
    let t_load = t_start.elapsed();

    log::info!("Ready in {:}ms", t_load.as_millis());

    println!("Atari serial disk: READY.");
    println!("Press ^C to exit.");
    state_machine::run(&disk_layout, &storage, &mut serial)?;

    Ok(())
}
