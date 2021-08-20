use std::{
    fmt::Debug,
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
};

use ataridisk::{entries::FileInfo, storage::DiskStorage};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opt {
    /// Dump file to load data from
    src_filename: PathBuf,

    /// Folder to dump data to
    #[structopt(default_value = "out")]
    dst_folder: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    env_logger::init();

    log::info!("Reading dump file from {:?}", opt.src_filename);
    let src_file = File::open(opt.src_filename)?;
    let reader = BufReader::new(src_file);
    let disk: DiskStorage = bincode::deserialize_from(reader)?;

    log::info!("Dumping disk content to: {:?}", opt.dst_folder);
    fs::create_dir_all(&opt.dst_folder)?;

    for file_info in disk.list_root_file_infos() {
        if file_info.is_dir() {
            dump_dir(&disk, &file_info, &opt.dst_folder)?;
        } else {
            dump_file(&disk, &file_info, &opt.dst_folder)?;
        }
    }
    Ok(())
}

fn dump_file<P>(disk: &DiskStorage, file_info: &FileInfo, out_dir: P) -> anyhow::Result<()>
where
    P: AsRef<Path>,
{
    let file_name = out_dir.as_ref().join(&file_info.filename()?);

    log::info!("Dumping: {:?}", file_name);
    let content = disk.read_file(file_info)?;
    fs::write(file_name, content)?;
    Ok(())
}

fn dump_dir<P>(disk: &DiskStorage, file_info: &FileInfo, out_dir: P) -> anyhow::Result<()>
where
    P: AsRef<Path>,
{
    let out_dir = out_dir.as_ref().join(file_info.filename()?);
    fs::create_dir_all(&out_dir)?;

    for entry in disk.read_dir(file_info)?.iter().skip(2) {
        if entry.is_dir() {
            dump_dir(disk, entry, &out_dir)?;
        } else {
            dump_file(disk, entry, &out_dir)?;
        }
    }
    Ok(())
}
