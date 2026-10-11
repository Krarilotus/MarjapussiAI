fn main() {
    if let Err(error) = tools::selfplay::run_with(tools::selfplay::builtin_policies()) {
        eprintln!("selfplay: {error}");
        std::process::exit(1);
    }
}
