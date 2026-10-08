use beliefs::BeliefParams;

#[test]
fn nested_model_parameters_reject_invalid_domains() {
    let invalid: &[fn(&mut BeliefParams)] = &[
        |p| p.bands.low = f32::NAN,
        |p| p.bands.high = 1.1,
        |p| p.bands.low = p.bands.high + 0.1,
        |p| p.factors.human.pass[0][0] = -1.0,
        |p| p.factors.agent.lead[0] = f32::INFINITY,
        |p| p.kernel.human.stay[0][0] = -0.1,
        |p| p.kernel.agent.stay[0][0] = f32::NAN,
        |p| p.kernel.agent.stay[0][0] = 1.1,
        |p| p.calibration.temperature[0] = 0.0,
        |p| p.calibration.temperature[0] = -1.0,
        |p| p.calibration.temperature[0] = f32::INFINITY,
    ];
    for (case, change) in invalid.iter().enumerate() {
        let mut params = BeliefParams::default();
        change(&mut params);
        assert!(params.validate().is_err(), "case {case}");
    }
}

#[test]
fn zero_factors_and_probability_endpoints_are_valid() {
    let mut params = BeliefParams::default();
    params.bands.low = 0.0;
    params.bands.high = 0.0;
    params.factors.human.pass[0][0] = 0.0;
    params.factors.agent.lead[0] = 2.0;
    params.kernel.human.stay[0][0] = 0.0;
    params.kernel.agent.stay[0][0] = 1.0;
    params.calibration.temperature[0] = 0.5;
    params.validate().unwrap();
}
