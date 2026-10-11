fn main() {
    if let Err(error) = tools::dryrun::run_with(tools::selfplay::builtin_policies()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
