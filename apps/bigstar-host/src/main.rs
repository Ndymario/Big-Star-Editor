//! bigstar-host: the command line verbs of the host. `serve` arrives with Phase 5.

mod cli;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = cli::run(&args, &mut std::io::stdout(), &mut std::io::stderr());
    std::process::exit(code);
}
