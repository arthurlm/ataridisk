use std::{thread::sleep, time::Duration};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use indicatif::ProgressIterator;
use serialport::SerialPort;

use crate::{checksum, error, layout::DiskLayout, storage::DiskStorage};

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
    ReceiveData,
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
            Self::ReceiveData => 1,
        }
    }
}

fn read_sector_infos(buffer: &[u8]) -> (u16, u16) {
    let index = ((buffer[0] as u16) << 8) + buffer[1] as u16;
    let count = ((buffer[2] as u16) << 8) + buffer[3] as u16;

    log::info!("sector index={:#04x}, count={:#04x}", index, count);
    (index, count)
}

pub fn run<S>(
    disk_layout: &DiskLayout,
    storage: &mut DiskStorage,
    serial: &mut S,
) -> error::Result<()>
where
    S: SerialPort,
{
    let mut buffer = [0; 5];
    let mut state = SerialState::new();

    let mut receive_sector_index = 0;
    let mut receive_sector_count = 0;

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

                let mut data = Vec::with_capacity(
                    sector_count as usize * disk_layout.bytes_per_sector() as usize,
                );
                storage.read_sectors(&mut data, sector_index, sector_count)?;
                assert_eq!(data.capacity(), data.len(), "Out buffer not fully filled");

                write_buffer(serial, &data)?;

                SerialState::Waiting
            }

            // Write command
            SerialState::ReceiveWriteSector => {
                let (sector_index, sector_count) = read_sector_infos(&buffer);
                receive_sector_index = sector_index;
                receive_sector_count = sector_count;

                SerialState::ReceiveData
            }

            // Waiting for Atari data
            SerialState::ReceiveData => match buffer[0] {
                0x00 => {
                    // Reserve a buffer for all the data with need to read
                    let mut data = Vec::with_capacity(
                        disk_layout.bytes_per_sector() as usize * receive_sector_count as usize,
                    );

                    // Read the data from Atari over serial port
                    log::info!("Reading data from Atari (bytes count: {})", data.capacity());
                    for _ in (0..data.capacity()).progress() {
                        data.push(serial.read_u8()?);
                    }

                    // Read the CRC32
                    let valid_crc = checksum::check_crc32(serial, &data)?;
                    if valid_crc {
                        serial.write_u8(0x01)?;

                        storage.write_sectors(
                            &mut data.as_slice(),
                            receive_sector_index,
                            receive_sector_count,
                        )?;

                        SerialState::Waiting
                    } else {
                        serial.write_u8(0x00)?;

                        SerialState::ReceiveData
                    }
                }
                0x1F => unimplemented!("read data with RLE compression"),
                _ => {
                    clear_serial(serial)?;
                    SerialState::Waiting
                }
            },
        };
    }
}

fn write_buffer<W>(writer: &mut W, data: &[u8]) -> error::Result<()>
where
    W: WriteBytesExt,
{
    let compressed = lz4_flex::compress(data);

    // Write flags (0 = no compression, 1 = lz4 compression)
    let send_compressed = compressed.len() < data.len();
    let flags = if send_compressed { 0x01 } else { 0x00 };
    writer.write_u8(flags)?;

    if send_compressed {
        // Write data compressed
        writer.write_u32::<BigEndian>(compressed.len() as u32)?;
        write_buffer_content(writer, &compressed)?;
    } else {
        // Write data uncompressed
        write_buffer_content(writer, data)?;
    }

    // Write checksum
    checksum::write_crc32(writer, data)?;

    Ok(())
}

fn write_buffer_content<W>(writer: &mut W, data: &[u8]) -> error::Result<()>
where
    W: WriteBytesExt,
{
    log::info!("Sending data (buffer size: {} bytes)", data.len());

    for idx in (0..data.len()).progress() {
        writer.write_u8(data[idx])?;
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
