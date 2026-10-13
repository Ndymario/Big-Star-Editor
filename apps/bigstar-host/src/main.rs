//! bigstar-host: the headless host (`serve`) and the command line verbs.

mod cli;
mod oracle;
mod server;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = cli::run(&args, &mut std::io::stdout(), &mut std::io::stderr());
    std::process::exit(code);
}
