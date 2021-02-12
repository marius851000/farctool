use anyhow::{bail, Context, Result};
use clap::Clap;
use env_logger;
use log::{debug, info};
use pmd_farc::{hash_name, message_dehash, Farc, FarcWriter, FileHashType};
use std::fs::File;
use std::path::PathBuf;
use std::{
    fs::create_dir_all,
    io::{Read, Write},
};

#[derive(Clap)]
/// tool for reading farc file (PSMD/GTI archive file)
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    Read(ReadParameter),
    Write(WriteParameter),
}

/// commands that read an input farc file
#[derive(Clap)]
struct ReadParameter {
    /// a path to the input farc file
    input: PathBuf,
    #[clap(short, long)]
    /// try to \"brute-force\" to file name (isn't a brute force per-see, more try to find in other part of the romfs, if present)
    brute: bool,
    #[clap(subcommand)]
    subcmd: ReadSubCommand,
}

#[derive(Clap)]
enum ReadSubCommand {
    Info(InfoParameter),
    Extract(ExtractParameter),
}

#[derive(Clap)]
/// display some information about the given farc file
struct InfoParameter {}

#[derive(Clap)]
/// extract the given farc file to a directory
struct ExtractParameter {
    /// a path to the folder in which the files are extracted
    output: PathBuf,
}

/// create a new farc file from extracted data in the input folder
#[derive(Clap)]
struct WriteParameter {
    /// a path to the folder that contain file to compress
    input: PathBuf,
    /// a path tot he output farc file
    output: PathBuf,
}

fn main() -> Result<()> {
    env_logger::init();
    let opts = Opts::parse();

    match opts.subcmd {
        SubCommand::Read(read_parameter) => {
            let input_file = File::open(&read_parameter.input)?;
            let input_name = read_parameter
                .input
                .file_name()
                .context("unable to get the file name of the FARC file")?
                .to_str()
                .context("can't convert the FARC file name to an UTF-8 string")?;
            let mut farc = Farc::new(input_file).context("unable to parse the FARC file")?;
            if read_parameter.brute {
                if farc.file_unknown_name() == 0 {
                    println!("all name information contained in file. No need to search for them.")
                } else {
                    println!("trying to find the name of files");
                    match FileHashType::predict_from_file_name(input_name) {
                        Some(FileHashType::Message) => {
                            if let Some(lst_file_name) = message_dehash::get_file_name(input_name) {
                                let lst_file_path =
                                    &read_parameter.input.with_file_name(lst_file_name);
                                match File::open(lst_file_path) {
                                    Ok(mut lst_file) => if let Err(err) = message_dehash::try_possible_name(&mut farc, &mut lst_file) {
                                        println!("ERROR: despite being able to locate and open the list file ({:?}), it did had an error while reading: {}", lst_file_path, err);
                                    },
                                    Err(err) => println!("ERROR: can't open the list at {:?}, it can't be opened due to the following error: {}", lst_file_path, err),
                                };
                            } else {
                                println!(
                                    "ERROR: can't get the name of the associated list for {:?}",
                                    input_name
                                );
                            }
                        }
                        None => println!("do not know how to get file name of this archive"),
                    };
                    match farc.file_unknown_name() {
                        0 => println!("all name hash were found"),
                        remaining => println!("unable to find file name for {} files", remaining),
                    }
                }
            }

            match read_parameter.subcmd {
                ReadSubCommand::Info(_) => {
                    info!("displaying info");
                    println!("file with known name :");
                    for name in farc.iter_name() {
                        println!("  {}", name);
                    }
                    println!("file without known name :");
                    for crc in farc.iter_hash_unknown_name() {
                        println!("  {}", crc);
                    }
                    println!("file count: {}", farc.file_count());
                }
                ReadSubCommand::Extract(extract_parameter) => {
                    info!("extracting file to {:?}", extract_parameter.output);
                    create_dir_all(&extract_parameter.output)?;
                    for name in farc.iter_name() {
                        let out_file_path = extract_parameter.output.join(name);
                        debug!("  extracting {:?} ...", out_file_path);
                        let mut stored_file = farc.get_named_file(name)?;
                        let mut in_memory_copy = Vec::new();
                        stored_file.read_to_end(&mut in_memory_copy)?;
                        let mut out_file = File::create(out_file_path)?;
                        out_file.write_all(&in_memory_copy)?;
                    }

                    for hash in farc.iter_hash_unknown_name().cloned() {
                        let name = format!("{:?}.bchunk", hash);
                        let out_file_path = extract_parameter.output.join(name);
                        debug!("  extracting {} ...", out_file_path.to_string_lossy());
                        let mut stored_file = farc.get_hashed_file(hash)?;
                        let mut in_memory_copy = Vec::new();
                        stored_file.read_to_end(&mut in_memory_copy)?;
                        let mut out_file = File::create(out_file_path)?;
                        out_file.write_all(&in_memory_copy)?;
                    }
                }
            }
        }
        SubCommand::Write(wp) => write(wp)?,
    };

    Ok(())
}

fn write(wp: WriteParameter) -> Result<()> {
    // 1. create the farc writer
    let mut farc_writer = FarcWriter::default();
    for entry_maybe in wp.input.read_dir()? {
        let entry = entry_maybe?;
        let file_name_os = entry.file_name();
        let full_path = wp.input.join(file_name_os.clone());
        let file_name = file_name_os.to_str().with_context(|| {
            format!(
                "the file name {:?} contain invalid utf8 character",
                entry.file_name()
            )
        })?;
        if !entry.file_type()?.is_file() {
            bail!("the file {:?} isn't a file", full_path);
        };
        let mut file = File::open(&full_path)?;
        let mut file_buffer = Vec::new();
        file.read_to_end(&mut file_buffer)?;
        let mut point_iter = file_name.split('.');
        let name_hash: u32 = (if let Some(first_part) = point_iter.next() {
            if let Some("bchunk") = point_iter.next() {
                if let Ok(hash) = first_part.parse() {
                    Some(hash)
                } else {
                    bail!("impossible to transform the text {:?} to a 32 bit number (a crc32 hash), for the file at {:?}", first_part, full_path);
                }
            } else {
                None
            }
        } else {
            None
        }).unwrap_or_else(|| hash_name(file_name));
        farc_writer.add_hashed_file(name_hash, file_buffer);
    }

    // 2. write it
    let mut out_file = File::create(&wp.output)?;
    farc_writer.write_hashed(&mut out_file)?;

    println!("file wrote !");
    Ok(())
}
