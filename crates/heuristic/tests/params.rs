use heuristic::Params;

#[test]
fn every_float_setting_rejects_nonfinite_input() {
    // Exercise the public loader, including newly added float settings in future defaults.
    for line in Params::default().to_toml().lines() {
        let (key, value) = line.split_once(" = ").unwrap();
        if matches!(value, "true" | "false")
            || matches!(key, "seed" | "tell_window" | "endgame_tricks" | "endgame_worlds")
        {
            continue;
        }
        for bad in ["NaN", "inf", "-inf", "1e100"] {
            assert!(Params::from_toml(&format!("{key} = {bad}")).is_err(), "{key} = {bad}");
        }
    }
}

#[test]
fn probability_thresholds_and_sampling_temperature_have_distinct_domains() {
    for text in [
        "trust_partner = -0.1",
        "conf_general = 1.1",
        "literal_share = 1.1",
        "partner_half_conf = -0.1",
        "over_140_cover = 1.1",
        "sample_temperature = -0.1",
        "convention_floor = 0",
        "convention_floor = 1.1",
    ] {
        assert!(Params::from_toml(text).is_err(), "{text}");
    }
    for text in [
        "trust_partner = 0\ntrust_opponent = 1",
        "partner_half_conf = 0\nover_140_cover = 1",
        "convention_floor = 1",
        "sample_temperature = 0",
        "sample_temperature = 2",
    ] {
        assert!(Params::from_toml(text).is_ok(), "{text}");
    }
}

#[test]
fn joint_bid_masses_are_validated_after_all_overrides() {
    for key in ["block_prior", "block_base"] {
        assert!(Params::from_toml(&format!("{key} = 0.8\nclaim_140_prior = 0.3")).is_err());
        // The first assignment is incompatible with the default claim mass, but the
        // final configuration is valid. Loading order must not change that result.
        let a = Params::from_toml(&format!("{key} = 0.75\nclaim_140_prior = 0.25")).unwrap();
        let b = Params::from_toml(&format!("claim_140_prior = 0.25\n{key} = 0.75")).unwrap();
        assert_eq!(a, b);
    }
}

#[test]
fn deliberate_zero_settings_remain_supported() {
    let text = "tell_window = 0\nendgame_tricks = 0\nendgame_worlds = 0\n\
                sample_temperature = 0\nseed = 18446744073709551615";
    let p = Params::from_toml(text).unwrap();
    assert_eq!(Params::from_toml(&p.to_toml()).unwrap(), p);
}

#[test]
fn obsolete_bid_model_settings_are_rejected_as_unknown() {
    for (key, value) in [
        ("read_hp1_as_hp1", "false"),
        ("bid_overreach", "0"),
        ("take_margin", "10"),
        ("hp1_info_margin", "0"),
        ("egp_base", "60"),
        ("egp_ace", "7"),
        ("egp_ten", "3"),
        ("egp_partner_ace", "10"),
        ("egp_half", "0"),
        ("egp_long", "0"),
        ("egp_void", "0"),
        ("egp_meld", "1"),
        ("egp_second_pair_share", "0.5"),
    ] {
        let err = Params::from_toml(&format!("{key} = {value}")).unwrap_err();
        assert_eq!(err.to_string(), format!("params: unknown key {key}"));
    }
}

#[test]
fn enabled_endgames_need_worlds_and_cannot_exceed_the_games_trick_count() {
    assert!(Params::from_toml("endgame_worlds = 0").is_err());
    assert!(Params::from_toml(&format!("endgame_tricks = {}", engine::TRICKS + 1)).is_err());
    assert!(Params::from_toml(&format!("endgame_tricks = {}\nendgame_worlds = 1", engine::TRICKS)).is_ok());
}

#[test]
fn rust_configuration_validation_includes_nested_beliefs() {
    let mut params = Params::default();
    params.beliefs.calibration.temperature[0] = 0.0;
    assert!(params.validate().is_err());
}
