//! Dedicated native Expat process. No Python backend or external resolver.
fn main() {
    if let Err(error) = ssx_infra::adapters::arxiv_expat::helper_main() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
