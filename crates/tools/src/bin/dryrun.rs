fn main() {
    let recorder = tools::selfplay::AssessmentRecorder::default();
    let policies = tools::selfplay::builtin_policies_with_recording(recorder.clone());
    if let Err(error) = tools::dryrun::run_with_recording(policies, None, Some(recorder)) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
