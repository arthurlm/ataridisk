# Atari serial disk rust

This project is a port of [SerialDisk](https://github.com/z80andrew/SerialDisk) in [Rust](https://www.rust-lang.org).

There is no real purposes on this app except:

- Have some fun with Atari ST network capabilities.
- Play with rust as a low level language.
- Understand how FAT12 / FAT16 works.
- Implement a virtual hard disk.

## State of the project

The app for now:

- fully expose folder as a RAM disk with READ + WRITE capabilities (using `ataridisk` utility)
- allow dump of a RAM disk as a real folder (using `dump2disk` utility)

## How this project differs from SerialDisk

There is few differences between SerialDisk implementation and this implementation.

- There is no FS mapping here. All FS content is load into memory.
  Nowaday with have huge amount of memory. Max Atari folder size is 512Mb.
  We can mount this as a RAM disk.

- Memory management here is more strict.
  Cluster / sector / bytes are not managed as a big chunk of `u8`.
  It is manage as dedicated struct.
  So app may panic easily if Atari ask for an uninitialized cluster.

- App is less configurable. This allow better optimization at compile time.

- LZ4 compression is enabled / disabled on the fly. So if you are sending already
  compressed data (ex: `*.MSA`), then compression will be disabled depending of
  buffers size.

Again this project is just here to have fun with Atari ST hardware :wink:.

## Configuration

See `config.json` and `--help` option.
