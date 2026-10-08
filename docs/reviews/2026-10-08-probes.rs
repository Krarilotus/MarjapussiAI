use heuristic::Params;

#[test]
fn nonfinite_probability_parameters_must_be_rejected() {
    assert!(Params::from_toml("trust_partner = NaN").is_err());
}
