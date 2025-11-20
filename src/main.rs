use clap::{CommandFactory, Parser};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, exit};
use std::{fs, io, str};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Path of pdf to split
    #[arg(value_name = "path/to/pdf")]
    input_file: Option<PathBuf>,

    /// Directory to put split images
    #[arg(
        short = 'o',
        long = "output-dir",
        value_name = "path/to/output_directory",
        default_value = "./output/"
    )]
    output_dir: PathBuf,

    /// Renames all files in the output directory from `*s.png` => `*.png`
    #[arg(short = 'r', long = "revert", default_value_t = false)]
    revert: bool,

    /// Renames files that you want to be spreads from `*.png` => `*s.png`, input as follows: 1,2,31,32
    #[arg(short, long, value_name = "page numbers", num_args = 1.., value_delimiter = ',')]
    spreads: Option<Vec<u8>>,
}

fn split_pdf(input_pdf: &PathBuf, output_dir: &PathBuf) -> Result<(), ()> {
    let input_file_str = input_pdf
        .to_str()
        .expect("File name should be valid unicode");
    let output_dir_str = output_dir
        .to_str()
        .expect("Directory name should be valid unicode");

    // validate input file
    if Path::new(&input_pdf).exists() == false {
        panic!("ERROR: file `{}` doesn't exist.", input_file_str);
    }

    let mut stdin_buf = String::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    // check output dir is empty, if it isn't check it's ok
    // TODO: -y flag
    let output_dir = Path::new(output_dir);
    if output_dir.exists() {
        let files = match fs::read_dir(&output_dir) {
            Ok(x) => x,
            Err(e) => {
                panic!("ERROR: Failed to read the contents of `{output_dir_str}`: {e}");
            }
        };

        if files.count() > 0 {
            loop {
                print!("WARNING: `{}` not empty, continue? [y/N] ", output_dir_str);
                if let Err(e) = stdout.flush() {
                    panic!("ERROR: Couldn't flush stdout: {e}");
                }

                if let Err(e) = stdin.read_line(&mut stdin_buf) {
                    panic!("ERROR: Couldn't read line: {e}");
                }

                match stdin_buf.as_str() {
                    "y\n" | "Y\n" => break,
                    _ => {}
                }

                stdin_buf.clear();
            }
        }
    }

    // TODO: change OS version based on host machine
    let cpdf_path = "./cpdf/Linux-Intel-64bit/cpdf";
    let cpdf_path = fs::canonicalize(&cpdf_path).expect("Path to cpdf should always exist");

    let mut cpdf = Command::new(&cpdf_path);
    let cpdf = cpdf.args([
        input_file_str,
        "-gs-quiet",
        "-gs",
        "gs",
        "-rasterize-res",
        "300",
        "-output-image",
        input_file_str,
        "-o",
        format!("{}/@N.png", output_dir_str).as_str(),
    ]);

    let cpdf_output = match cpdf.output() {
        Ok(x) => x,
        Err(e) => {
            panic!("ERROR: cpdf failed: {e:?}");
        }
    };

    if !cpdf_output.status.success() {
        eprintln!("ERROR: cpdf failed:");
        let cpdf_stdout = match str::from_utf8(cpdf_output.stdout.as_slice()) {
            Ok(x) => x,
            Err(e) => {
                panic!("ERROR: cpdf stdout is not UTF-8: {e}");
            }
        };

        let cpdf_stderr = match str::from_utf8(cpdf_output.stderr.as_slice()) {
            Ok(x) => x,
            Err(e) => {
                panic!("ERROR: cpdf stderr is not UTF-8: {e}");
            }
        };

        eprintln!("cpdf captured stdout: {cpdf_stdout}");
        eprintln!("cpdf captured stderr: {cpdf_stderr}");
        exit(1);
    }

    Ok(())
}

fn revert_page_spreads(output_dir: &PathBuf) -> Result<(), ()> {
    let output_dir_str = output_dir
        .to_str()
        .expect("Directory name should be valid unicode");
    let files = match fs::read_dir(&output_dir) {
        Ok(x) => x,
        Err(e) => {
            panic!("ERROR: Failed to read the contents of `{output_dir_str}`: {e}");
        }
    };

    for file in files.into_iter() {
        let file = match file {
            Ok(x) => x,
            Err(e) => {
                panic!("ERROR: Couldn't read file: {e}");
            }
        };

        let file_name = file
            .file_name()
            .into_string()
            .expect("File name should be valid unicode");
        let file_number = match file_name.strip_suffix("s.png") {
            Some(x) => x,
            None => continue,
        };

        let from = format!("{}/{}", output_dir_str, file_name);
        let to = format!("{}/{}.png", output_dir_str, file_number);
        if let Err(e) = fs::rename(&from, &to) {
            eprintln!("WARNING: Failed to rename `{from}` to `{to}`: {e:?}");
        }
    }

    Ok(())
}

fn rename_page_spreads(input_spreads: &Vec<u8>, output_dir: &PathBuf) -> Result<(), ()> {
    let output_dir_str = output_dir
        .to_str()
        .expect("Directory name should be valid unicode");

    let spreads_out: Vec<&u8> = input_spreads
        .iter()
        .filter(|x| {
            if !input_spreads.contains(&(*x + 1)) && !input_spreads.contains(&(*x - 1)) {
                println!("WARNING: {x} has no neighbors, will not be marked as a page spread.");
                return false;
            }

            return true;
        })
        .collect();

    spreads_out.iter().for_each(|x| {
        let from = format!("{output_dir_str}/{x}.png");
        let to = format!("{output_dir_str}/{x}s.png");
        if let Err(e) = fs::rename(&from, &to) {
            eprintln!("WARNING: Failed to rename `{from}` to `{to}`: {e:?}");
        }
    });

    Ok(())
}

fn main() -> Result<(), ()> {
    let cli = Cli::parse();

    // theres probably a better way to do this,.,
    if let (None, None) = (&cli.input_file, &cli.spreads) {
        let mut cmd = Cli::command();
        cmd.print_help().expect("This is not a debug build");
        exit(0);
    }

    // renames `*s.png` => `*.png`
    if cli.revert {
        revert_page_spreads(&cli.output_dir)?;
    }

    // uses cpdf to split the pdf into individual pages
    if let Some(input_file) = &cli.input_file {
        split_pdf(&input_file, &cli.output_dir)?;
    }

    // renames given files from `*.png` => `*s.png`
    if let Some(spreads) = &cli.spreads {
        rename_page_spreads(&spreads, &cli.output_dir)?;
    }

    Ok(())
}
