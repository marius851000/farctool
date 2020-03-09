#[macro_use]
extern crate log;
use clap::{App, Arg, SubCommand};
use env_logger;
use pmd_farc::Farc;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

fn main() {
    env_logger::init();
    let matches = App::new("farctool")
        .about("tool for reading farc file (PSMD/GTI archive file)")
        .arg(
            Arg::with_name("input")
                .short("i")
                .long("input")
                .help("the input file of the program")
                .required(true)
                .value_name("INPUT"),
        )
        /*.arg(
            Arg::with_name("brute")
                .short("b")
                .long("brute")
                .help("try to \"brute-force\" to file name"),
        )*/
        .subcommand(SubCommand::with_name("info").about("display some information about the input"))
        .subcommand(
            SubCommand::with_name("extract")
                .about("extract the file to a directory")
                .arg(
                    Arg::with_name("output")
                        .short("o")
                        .long("output")
                        .help("the output directory")
                        .required(true)
                        .value_name("OUTPUT"),
                ),
        )
        .get_matches();

    let input_path = PathBuf::from(matches.value_of("input").unwrap());
    let input_file = File::open(&input_path).unwrap();
    //let input_name = input_path.file_name().unwrap();
    let farc = Farc::new(input_file).unwrap();

    /* if matches.is_present("brute") {
        info!("trying to find the name of files");
        if farc.file_count_hashed() == 0 {
            info!("all name information contained in file. No need to search for them.")
        } else {
            if input_name == "pokemon_graphic.bin" {
                let pgdb_file =
                    File::open(&input_path.with_file_name("pokemon_graphics_database.bin")).unwrap();
                    let mut pgdb = pmd_farc::Pgdb::new(pgdb_file).unwrap();
                pmd_farc::find_name_monster_graphic(&mut farc, &mut pgdb).unwrap();
            } else {
                warn!("no way to brute force the name found for this file !");
            };
            match farc.file_count_hashed() {
                0 => info!("all name hash were found"),
                remaining => info!("unable to find file name for {} files", remaining),
            }
        }
    }; */

    if matches.subcommand_matches("info").is_some() {
        info!("displaying info");
        println!("file count: {}", farc.file_count());
        println!("file with known name :");
        for name in farc.iter_name() {
            println!("  {}", name);
        }
        println!("file without known name :");
        for crc in farc.iter_hash() {
            println!("  {}", crc);
        }
    }

    if let Some(subcommand) = matches.subcommand_matches("extract") {
        let out_folder_string = subcommand.value_of("output").unwrap();
        info!("extracting file to {}", out_folder_string);
        let out_folder = PathBuf::from(out_folder_string);
        for name in farc.iter_name() {
            let out_file_path = out_folder.join(name);
            debug!("  extracting {} ...", out_file_path.to_string_lossy());
            let mut stored_file = farc.get_named_file(name).unwrap();
            let mut in_memory_copy = Vec::new();
            stored_file.read_to_end(&mut in_memory_copy).unwrap();
            let mut out_file = File::create(out_file_path).unwrap();
            out_file.write_all(&in_memory_copy).unwrap();
        }

        use std::fmt::Write; //TODO:
        for hash in farc.iter_hash() {
            let mut name = String::new();
            write!(&mut name, "{:?}.bchunk", hash).unwrap(); //TODO
            let out_file_path = out_folder.join(name);
            debug!("  extracting {} ...", out_file_path.to_string_lossy());
            let mut stored_file = farc.get_unnamed_file(hash).unwrap();
            let mut in_memory_copy = Vec::new();
            stored_file.read_to_end(&mut in_memory_copy).unwrap();
            let mut out_file = File::create(out_file_path).unwrap();
            out_file.write_all(&in_memory_copy).unwrap();
        }
    }
}
