//! Build-time protection configuration parsed from the CLI (the GUI supplies the same
//! structure from its checkbox panel — spec §17: independent toggles, never
//! radio modes).

use nyedarch_crypto::timewin::DailySchedule;

#[derive(Clone)]
pub struct ProtectionConfig {
    /// Optional time protection (spec §22).
    pub schedule: Option<DailySchedule>,
    /// Optional location protection tolerance in metres (spec §20).
    pub location_tolerance_m: Option<u32>,
    /// Execution policy (spec §37).
    pub one_shot: bool,
    /// Extra trusted fingerprint records to include (spec §6 OR-set).
    pub trust_files: Vec<String>,
    /// Select trusted machines from the registry by label (spec §48).
    pub trust_tags: Vec<String>,
    /// How multiple tags combine: ANY selected tag, or ALL of them.
    pub tag_mode_all: bool,
    /// Free-text filter over ids and labels.
    pub trust_query: String,
    /// Compression effort (spec §16).
    pub compression: nyedarch_package::pipeline::CompressionMode,
    /// Creator mode (spec §50): diagnostics only, never additional authority.
    pub creator_mode: bool,
    /// Hardware-backed machine protection policy.
    pub hardware: nyedarch_platform::hardware::HardwarePolicy,
}

impl Default for ProtectionConfig {
    fn default() -> Self {
        Self {
            schedule: None,
            location_tolerance_m: None,
            one_shot: false,
            trust_files: Vec::new(),
            trust_tags: Vec::new(),
            tag_mode_all: false,
            trust_query: String::new(),
            compression: nyedarch_package::pipeline::CompressionMode::Automatic,
            creator_mode: false,
            hardware: nyedarch_platform::hardware::HardwarePolicy::Preferred,
        }
    }
}

/// Parse `HH:MM` into minutes past midnight.
fn parse_hhmm(s: &str) -> Result<u32, String> {
    let (h, m) = s.split_once(':').ok_or_else(|| format!("bad time '{s}', expected HH:MM"))?;
    let h: u32 = h.trim().parse().map_err(|_| format!("bad hour in '{s}'"))?;
    let m: u32 = m.trim().parse().map_err(|_| format!("bad minute in '{s}'"))?;
    if h > 23 || m > 59 {
        return Err(format!("time out of range: '{s}'"));
    }
    Ok(h * 60 + m)
}

/// Parse protection flags. Unknown flags are rejected rather than ignored, so a
/// mistyped security flag can never silently disable a protection (spec §36).
pub fn parse(args: &[String]) -> Result<ProtectionConfig, String> {
    let mut cfg = ProtectionConfig::default();
    let mut slots: Vec<u32> = Vec::new();
    let mut tol_min: u32 = 15;
    let mut tz: i32 = 0;
    let mut time_enabled = false;

    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let next = |i: &mut usize| -> Result<String, String> {
            *i += 1;
            args.get(*i).cloned().ok_or_else(|| format!("{a} requires a value"))
        };
        match a {
            "--time" => {
                time_enabled = true;
                let v = next(&mut i)?;
                for part in v.split(',') {
                    slots.push(parse_hhmm(part.trim())?);
                }
            }
            "--time-tolerance-min" => {
                tol_min = next(&mut i)?.parse().map_err(|_| "bad tolerance".to_string())?;
            }
            "--tz-offset-min" => {
                tz = next(&mut i)?.parse().map_err(|_| "bad tz offset".to_string())?;
            }
            "--location" => {
                let v = next(&mut i)?;
                let m: u32 = v.parse().map_err(|_| "bad location tolerance".to_string())?;
                if m == 0 {
                    return Err("location tolerance must be > 0".into());
                }
                cfg.location_tolerance_m = Some(m);
            }
            "--one-shot" => cfg.one_shot = true,
            "--creator" => cfg.creator_mode = true,
            "--hardware" => {
                let v = next(&mut i)?;
                cfg.hardware = nyedarch_platform::hardware::HardwarePolicy::parse(&v)
                    .ok_or_else(|| format!("--hardware must be 'preferred' or 'required', not '{v}'"))?;
            }
            "--compression" => {
                let v = next(&mut i)?;
                cfg.compression = nyedarch_package::pipeline::CompressionMode::parse(&v)
                    .ok_or_else(|| format!("--compression must be automatic, maximum, balanced or fast, not '{v}'"))?;
            }
            "--trust" => cfg.trust_files.push(next(&mut i)?),
            "--trust-tag" => cfg.trust_tags.push(next(&mut i)?),
            "--trust-search" => cfg.trust_query = next(&mut i)?,
            "--tag-mode" => {
                let v = next(&mut i)?;
                cfg.tag_mode_all = match v.to_ascii_lowercase().as_str() {
                    "all" => true,
                    "any" => false,
                    other => return Err(format!("--tag-mode must be 'any' or 'all', not '{other}'")),
                };
            }
            other => return Err(format!("unknown flag '{other}'")),
        }
        i += 1;
    }

    if time_enabled {
        if slots.is_empty() {
            return Err("--time given with no slots".into());
        }
        if tol_min == 0 {
            return Err("time tolerance must be > 0".into());
        }
        cfg.schedule = Some(DailySchedule { slots_minutes: slots, tolerance_minutes: tol_min, tz_offset_minutes: tz });
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_hardware_policy() {
        use nyedarch_platform::hardware::HardwarePolicy;
        assert_eq!(parse(&v(&["--hardware", "required"])).unwrap().hardware, HardwarePolicy::Required);
        // The default must be the safe-to-run one, with the downgrade reported.
        assert_eq!(parse(&v(&[])).unwrap().hardware, HardwarePolicy::Preferred);
        // A typo must not silently weaken the policy.
        assert!(parse(&v(&["--hardware", "optional"])).is_err());
    }

    #[test]
    fn parses_compression_and_creator_mode() {
        use nyedarch_package::pipeline::CompressionMode;
        let c = parse(&v(&["--compression", "maximum", "--creator"])).unwrap();
        assert_eq!(c.compression, CompressionMode::Maximum);
        assert!(c.creator_mode);
        // Default is automatic, and creator mode is off unless asked for.
        let d = parse(&v(&[])).unwrap();
        assert_eq!(d.compression, CompressionMode::Automatic);
        assert!(!d.creator_mode);
    }

    #[test]
    fn rejects_an_unknown_compression_mode() {
        assert!(parse(&v(&["--compression", "extreme"])).is_err());
    }

    #[test]
    fn parses_tag_selection() {
        let c = parse(&v(&["--trust-tag", "HR", "--trust-tag", "Bangalore", "--tag-mode", "all", "--trust-search", "employee"])).unwrap();
        assert_eq!(c.trust_tags, vec!["HR", "Bangalore"]);
        assert!(c.tag_mode_all);
        assert_eq!(c.trust_query, "employee");
    }

    #[test]
    fn rejects_an_unknown_tag_mode() {
        // A typo must not silently fall back to the looser ANY semantics, which
        // would trust more machines than the operator asked for.
        assert!(parse(&v(&["--tag-mode", "either"])).is_err());
    }

    #[test]
    fn parses_all_protections() {
        let c = parse(&v(&["--time", "14:00,02:30", "--time-tolerance-min", "20", "--tz-offset-min", "330", "--location", "150", "--one-shot"])).unwrap();
        let s = c.schedule.unwrap();
        assert_eq!(s.slots_minutes, vec![840, 150]);
        assert_eq!(s.tolerance_minutes, 20);
        assert_eq!(s.tz_offset_minutes, 330);
        assert_eq!(c.location_tolerance_m, Some(150));
        assert!(c.one_shot);
    }

    #[test]
    fn rejects_bad_input_rather_than_disabling_a_protection() {
        assert!(parse(&v(&["--typo-location", "100"])).is_err());
        assert!(parse(&v(&["--time", "99:99"])).is_err());
        assert!(parse(&v(&["--location", "0"])).is_err());
        assert!(parse(&v(&["--time"])).is_err());
    }
}
