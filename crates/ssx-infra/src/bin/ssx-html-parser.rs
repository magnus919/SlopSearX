fn main() {
    if let Err(error) = ssx_infra::html::helper_main() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
