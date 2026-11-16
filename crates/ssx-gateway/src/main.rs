use clap::Parser;
#[derive(Parser)]
#[command(about = "Native foundation entrypoint; commands are not implemented")]
struct Args {
    command: Option<String>,
}
fn main() -> std::process::ExitCode {
    let _args = Args::parse();
    eprintln!("not implemented: native feature work is pending");
    std::process::ExitCode::FAILURE
}
