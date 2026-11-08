use clap::Parser;
use cppm::{Cli, CommandFailure};

fn main() {
    if let Err(error) = Cli::parse().execute() {
        eprintln!("cppm: {error:#}");
        let code = error.downcast_ref::<CommandFailure>().map_or(1, |e| e.0);
        std::process::exit(code);
    }
}
