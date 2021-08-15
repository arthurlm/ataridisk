use std::{thread::sleep, time::Duration};

use indicatif::ProgressIterator;
use serialport::SerialPort;

use crate::{checksum, error, layout::DiskLayout, network::NetworkEncode, storage::DiskStorage};

const BUF_MAGIC_START: [u8; 4] = [0x18, 0x03, 0x20, 0x06];

/*
macro_rules! print_buffer {
    ($buffer:expr) => {
        for i in 0..$buffer.len() {
            print!("{:#04x} ", $buffer[i]);
        }
        println!();
    };
}
*/

/// Possible communication state of hard disk vs Atari
#[derive(Debug)]
enum SerialState {
    Waiting,
    ReceiveReadSector,
    ReceiveWriteSector,
}

impl Default for SerialState {
    fn default() -> Self {
        SerialState::Waiting
    }
}

impl SerialState {
    fn new() -> Self {
        Self::default()
    }

    fn expected_buffer_len(&self) -> usize {
        match &*self {
            Self::Waiting => 5,
            Self::ReceiveReadSector | Self::ReceiveWriteSector => 4,
        }
    }
}

fn read_sector_infos(buffer: &[u8]) -> (u16, u16) {
    let index = ((buffer[0] as u16) << 8) + buffer[1] as u16;
    let count = ((buffer[2] as u16) << 8) + buffer[3] as u16;

    log::info!("sector index={:#04x}, count={:#04x}", index, count);
    (index, count)
}

pub fn run<S>(disk_layout: &DiskLayout, storage: &DiskStorage, serial: &mut S) -> error::Result<()>
where
    S: SerialPort,
{
    let mut buffer = [0; 64];
    let mut state = SerialState::new();

    loop {
        log::info!("State: {:?}", state);

        let l = state.expected_buffer_len();
        if l > 0 {
            serial.read_exact(&mut buffer[0..l])?;
            // print_buffer!(buffer);
        }

        state = match &state {
            // Handle waiting for Atari commands
            SerialState::Waiting => {
                // Switch to new state
                match (&buffer[0..4], buffer[4]) {
                    (magic, 0) if magic == BUF_MAGIC_START => SerialState::ReceiveReadSector,
                    (magic, 1) if magic == BUF_MAGIC_START => SerialState::ReceiveWriteSector,
                    (magic, 2) if magic == BUF_MAGIC_START => {
                        // Send Atari disk layout
                        log::info!("Sending atari BIOS parameter block");

                        disk_layout.write_bios_parameter_block(serial)?;
                        SerialState::Waiting
                    }
                    _ => {
                        clear_serial(serial)?;
                        SerialState::Waiting
                    }
                }
            }

            // Read command
            SerialState::ReceiveReadSector => {
                let (sector_index, sector_count) = read_sector_infos(&buffer);

                let data = storage.read_sectors(sector_index, sector_count);
                write_buffer(serial, &data)?;

                SerialState::Waiting
            }

            // Write command
            SerialState::ReceiveWriteSector => {
                let (_sector_index, _sector_count) = read_sector_infos(&buffer);

                unimplemented!();
            }
        };
    }
}

fn write_buffer<S>(serial: &mut S, data: &[u8]) -> error::Result<()>
where
    S: SerialPort,
{
    let compressed = lz4_flex::compress(data);

    // Write flags (0 = no compression, 1 = lz4 compression)
    let send_compressed = compressed.len() < data.len();
    let flags = if send_compressed { 0x01 } else { 0x00 };
    serial.write_all(&[flags])?;

    if send_compressed {
        // Write data compressed
        serial.write_all(&compressed.len().encode_network())?;
        write_buffer_content(serial, &compressed)?;
    } else {
        // Write data uncompressed
        write_buffer_content(serial, data)?;
    }

    // Write checksum
    checksum::write_crc32(serial, data)?;

    Ok(())
}

fn write_buffer_content<S>(serial: &mut S, data: &[u8]) -> error::Result<()>
where
    S: SerialPort,
{
    log::info!("Sending data (buffer size: {} bytes)", data.len());

    for idx in (0..data.len()).progress() {
        serial.write_all(&[data[idx]])?;
    }

    Ok(())
}

fn clear_serial<S>(serial: &mut S) -> error::Result<()>
where
    S: SerialPort,
{
    log::warn!("Desync with atari. Clearing buffers and ignore command");

    // Give some time for new data to come
    sleep(Duration::from_millis(500));

    // Discard everything
    serial.clear(serialport::ClearBuffer::All)?;
    Ok(())
}
