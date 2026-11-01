#![allow(clippy::expect_used)]
use solvercraft::{catalog_inputs::*, math, simulation::resampling::VectorAddress, types::*};
use std::sync::atomic::{AtomicBool, Ordering};
fn address(trial: u64) -> VectorAddress {
    VectorAddress { seed: u64::MAX, binding: 9007199254740993, trial }
}
fn make_request() -> MakeInputRequest {
    MakeInputRequest {
        address: Some(address(3)),
        frequency_kind: FrequencyKind::LiteralOrContinuous,
        deductible: None,
        limit: None,
        limit_policy: None,
        limits: MakeInputLimits::default(),
    }
}
fn source() -> PublicationSource {
    PublicationSource {
        publisher: "Caller-owned risk team".into(),
        source_id: "model:9007199254740993".into(),
        source_version: "published-v1".into(),
    }
}
fn reference(name: &str, formula: &str) -> Option<ReferenceCall> {
    Some(ReferenceCall { name: name.into(), formula: formula.into(), source_version: "frontline-2025q1".into(), properties: vec![] })
}
fn record(name: &str, data: PublishedData) -> PublishedInput {
    PublishedInput {
        schema_version: INPUT_SCHEMA_VERSION,
        id: format!("model:{name}"),
        name: name.into(),
        source: source(),
        reference: None,
        notes: vec!["Publisher-provided payload; no vendor certification asserted".into()],
        data,
    }
}
fn library(records: Vec<PublishedInput>) -> InputLibrary {
    InputLibrary { schema_version: INPUT_SCHEMA_VERSION, records }
}
#[test]
fn psimakeinput_compounds_the_whole_expression_and_preserves_event_reads() -> Result<()> {
    let mut request = make_request();
    request.deductible = Some(1.0);
    let mut calls = Vec::new();
    let bindings = vec![
        EventBinding { id: 1, distribution: Distribution::Constant { value: 2.0 } },
        EventBinding { id: 2, distribution: Distribution::Constant { value: 3.0 } },
    ];
    let result = psi_make_input(
        &request,
        &bindings,
        &NoControl,
        |_| Ok(3.9),
        |context| {
            let first = context.sample(1)?;
            assert_eq!(first, context.sample(1)?);
            let second = context.sample(2)?;
            calls.push(context.address());
            Ok(first + 2.0 * second)
        },
    )?;
    assert_eq!(result.termination, Termination::Completed);
    assert_eq!(result.requested_events, Some(3));
    assert_eq!(result.completed_events, 3);
    assert_eq!(result.value, Some(21.0));
    assert_eq!(calls.iter().map(|a| a.occurrence).collect::<Vec<_>>(), vec![0, 1, 2]);
    assert!(calls.iter().all(|a| a.seed == u64::MAX && a.binding == 9007199254740993 && a.trial == 3));
    Ok(())
}
#[test]
fn psimakeinput_addresses_replay_and_repeat_reads_stably_but_resample_each_occurrence() -> Result<()> {
    let bindings = vec![EventBinding { id: u64::MAX, distribution: Distribution::Uniform { low: 0.0, high: 1.0 } }];
    let request = make_request();
    let run = || -> Result<(MakeInputResult, Vec<f64>)> {
        let mut values = vec![];
        let result = psi_make_input(
            &request,
            &bindings,
            &NoControl,
            |_| Ok(4.0),
            |event| {
                let value = event.sample(u64::MAX)?;
                assert_eq!(value.to_bits(), event.sample(u64::MAX)?.to_bits());
                values.push(value);
                Ok(value)
            },
        )?;
        Ok((result, values))
    };
    let (first, values) = run()?;
    let (second, replayed) = run()?;
    assert_eq!(values, replayed);
    assert_eq!(first.value, second.value);
    assert!(values.windows(2).any(|v| v[0] != v[1]));
    assert_eq!(first.evaluations, 9); // One frequency, four expressions and four actual draws.
    Ok(())
}
#[test]
fn psimakeinput_frequency_defaults_domains_and_explicit_limit_order() -> Result<()> {
    let mut request = make_request();
    let mut invoked = 0;
    let zero = psi_make_input(
        &request,
        &[],
        &NoControl,
        |_| Ok(0.9),
        |_| {
            invoked += 1;
            Ok(7.0)
        },
    )?;
    assert_eq!(zero.value, Some(0.0));
    assert_eq!(invoked, 0);
    request.frequency_kind = FrequencyKind::DiscreteSample;
    let clamped = psi_make_input(&request, &[], &NoControl, |_| Ok(-2.0), |_| Ok(7.0))?;
    assert_eq!(clamped.requested_events, Some(1));
    assert_eq!(clamped.value, Some(7.0));
    let fractional = psi_make_input(&request, &[], &NoControl, |_| Ok(1.5), |_| Ok(7.0))?;
    assert_eq!(fractional.error.as_ref().map(|e| e.code.as_str()), Some("frequency_semantics"));
    assert_eq!(fractional.value, None);
    request.frequency_kind = FrequencyKind::LiteralOrContinuous;
    for bad in [-1.0, f64::INFINITY, f64::NAN] {
        let bad = psi_make_input(&request, &[], &NoControl, |_| Ok(bad), |_| Ok(7.0))?;
        assert_eq!(bad.value, None);
        assert_eq!(bad.termination, Termination::NumericalFailure);
    }
    request.deductible = Some(3.0);
    request.limit = Some(4.0);
    assert!(psi_make_input(&request, &[], &NoControl, |_| Ok(2.0), |_| Ok(10.0)).is_err_and(|e| e.code == "ambiguous_limit_policy"));
    for (policy, expected) in [
        (LimitPolicy::PerOccurrenceAfterDeductible, 8.0),
        (LimitPolicy::PerOccurrenceBeforeDeductible, 2.0),
        (LimitPolicy::AggregateAfterDeductible, 4.0),
    ] {
        request.limit_policy = Some(policy);
        let result = psi_make_input(&request, &[], &NoControl, |_| Ok(2.0), |_| Ok(10.0))?;
        assert_eq!(result.value, Some(expected));
    }
    request.limit = None;
    request.limit_policy = None;
    request.deductible = Some(20.0);
    assert_eq!(psi_make_input(&request, &[], &NoControl, |_| Ok(2.0), |_| Ok(10.0))?.value, Some(-20.0));
    request.address = None;
    assert!(psi_make_input(&request, &[], &NoControl, |_| Ok(1.0), |_| Ok(1.0)).is_err_and(|e| e.code == "simulation_context_required"));
    Ok(())
}
#[test]
fn psimakeinput_work_cancel_domain_and_overflow_keep_honest_partial_state() -> Result<()> {
    let mut request = make_request();
    request.limits.max_evaluations = 3;
    let limited = psi_make_input(&request, &[], &NoControl, |_| Ok(5.0), |_| Ok(4.0))?;
    assert_eq!(limited.completed_events, 2);
    assert_eq!(limited.partial_sum, Some(8.0));
    assert_eq!(limited.value, None);
    assert_eq!(limited.termination, Termination::EvaluationLimit);
    request.limits = MakeInputLimits::default();
    request.limits.max_work = 2;
    let work = psi_make_input(&request, &[], &NoControl, |_| Ok(5.0), |_| Ok(4.0))?;
    assert_eq!(work.completed_events, 1);
    assert_eq!(work.error.as_ref().map(|e| e.code.as_str()), Some("work_limit"));
    request.limits = MakeInputLimits::default();
    request.limits.max_events = 2;
    let count = psi_make_input(&request, &[], &NoControl, |_| Ok(f64::MAX), |_| Ok(4.0))?;
    assert_eq!(count.completed_events, 0);
    assert_eq!(count.termination, Termination::IterationLimit);
    request.limits = MakeInputLimits::default();
    let mut calls = 0;
    let domain = psi_make_input(
        &request,
        &[],
        &NoControl,
        |_| Ok(3.0),
        |_| {
            calls += 1;
            if calls == 2 { Err(SolverError::new("expression_domain", "Rejected event")) } else { Ok(2.0) }
        },
    )?;
    assert_eq!(domain.completed_events, 1);
    assert_eq!(domain.partial_sum, Some(2.0));
    assert_eq!(domain.termination, Termination::NumericalFailure);
    struct Cancel(AtomicBool);
    impl RunControl for Cancel {
        fn is_cancelled(&self) -> bool {
            self.0.load(Ordering::SeqCst)
        }
        fn progress(&self, event: &ProgressEvent) {
            if event.iterations == 2 {
                self.0.store(true, Ordering::SeqCst);
            }
        }
    }
    let cancelled = psi_make_input(&request, &[], &Cancel(AtomicBool::new(false)), |_| Ok(5.0), |_| Ok(4.0))?;
    assert_eq!(cancelled.completed_events, 2);
    assert_eq!(cancelled.partial_sum, Some(8.0));
    assert_eq!(cancelled.termination, Termination::Cancelled);
    let mut occurrence = 0;
    let exact = psi_make_input(
        &request,
        &[],
        &NoControl,
        |_| Ok(3.0),
        |_| {
            occurrence += 1;
            Ok(if occurrence < 3 { f64::MAX } else { -f64::MAX })
        },
    )?;
    assert_eq!(exact.value, Some(f64::MAX));
    request.limit = Some(10.0);
    request.limit_policy = Some(LimitPolicy::AggregateAfterDeductible);
    let capped = psi_make_input(&request, &[], &NoControl, |_| Ok(2.0), |_| Ok(f64::MAX))?;
    assert_eq!(capped.termination, Termination::Completed);
    assert_eq!(capped.value, Some(10.0));
    assert_eq!(capped.partial_sum, None);
    request.limits.max_memory_bytes = 1;
    assert!(psi_make_input(&request, &[], &NoControl, |_| Ok(1.0), |_| Ok(1.0)).is_err_and(|e| e.code == "memory_limit"));
    Ok(())
}
#[test]
fn psicertified_resolves_only_caller_publications_and_named_sequential_sips() -> Result<()> {
    let sip = record("Stored", PublishedData::Sip { values: vec![3.0, 1.0, 4.0] });
    let slurp = record(
        "Paired",
        PublishedData::Slurp { rows: vec![vec![10.0, 20.0], vec![11.0, 21.0]], column_names: vec!["A".into(), "B".into()], default_sip: None },
    );
    let analytic = record("Analytic", PublishedData::Distribution { distribution: Distribution::Normal { mean: 2.0, std_dev: 1.0 } });
    let library = library(vec![sip, slurp, analytic]);
    let prepared = PreparedInputLibrary::new(&library, &CatalogLimits::default(), &NoControl)?;
    let scalar = prepared.resolve("Stored", None)?;
    let analytic = prepared.resolve("Analytic", None)?;
    assert_eq!(analytic.sample(Some(address(0)), &NoControl)?, analytic.sample(Some(address(0)), &NoControl)?);
    assert_ne!(analytic.sample(Some(address(0)), &NoControl)?, analytic.sample(Some(address(1)), &NoControl)?);
    assert_eq!(scalar.sample(Some(address(0)), &NoControl)?, 3.0);
    assert_eq!(scalar.sample(Some(address(1)), &NoControl)?, 1.0);
    assert!(scalar.sample(Some(address(3)), &NoControl).is_err_and(|e| e.code == "replay_index"));
    assert!(scalar.sample(None, &NoControl).is_err_and(|e| e.code == "simulation_context_required"));
    assert!(prepared.resolve("Stored", Some("A")).is_err());
    assert!(prepared.resolve("Unknown", None).is_err());
    assert!(prepared.resolve("Paired", None).is_err_and(|e| e.code == "missing_default_sip"));
    assert_eq!(prepared.resolve("Paired", Some("B"))?.sample(Some(address(1)), &NoControl)?, 21.0);
    assert!(prepared.resolve("Paired", Some("unknown")).is_err());
    assert!(prepared.resolve("Paired", Some("")).is_err());
    let mut with_default = library;
    let PublishedData::Slurp { default_sip, .. } = &mut with_default.records[1].data else { return Err(SolverError::new("test", "Wrong fixture")) };
    *default_sip = Some("B".into());
    let prepared = PreparedInputLibrary::new(&with_default, &CatalogLimits::default(), &NoControl)?;
    assert_eq!(prepared.resolve("Paired", None)?.sample(Some(address(0)), &NoControl)?, 20.0);
    Ok(())
}
#[test]
fn psidistinfo_preserves_arithmetic_and_log_parameter_names_formulas_and_metadata() -> Result<()> {
    let mut arithmetic = record("Arithmetic", PublishedData::Distribution { distribution: math::psi_aliases::lognormal_arithmetic(100.0, 20.0)? });
    arithmetic.id = "9007199254740993".into();
    arithmetic.reference = reference("PsiLogNormal", "PsiLogNormal(100,20,PsiName(\"Revenue\"))");
    arithmetic.reference.as_mut().expect("metadata").properties.push("PsiName(\"Revenue\")".into());
    let mut logarithmic = record("Log", PublishedData::Distribution { distribution: math::psi_aliases::lognormal_log(0.0, 0.5)? });
    logarithmic.reference = reference("PsiLogNorm2", "PsiLogNorm2(0,0.5)");
    let lib = PreparedInputLibrary::new(&library(vec![arithmetic.clone(), logarithmic]), &CatalogLimits::default(), &NoControl)?;
    let info = lib.resolve("Arithmetic", None)?;
    assert_eq!(psi_dist_info(info.info(), None)?, "PsiLogNormal");
    assert_eq!(psi_dist_info(info.info(), Some(0))?, "PsiLogNormal");
    assert_eq!(psi_dist_info(info.info(), Some(1))?, "PsiLogNormal(100,20,PsiName(\"Revenue\"))");
    assert_eq!(info.info().id, "9007199254740993");
    assert_eq!(info.info().source.source_version, "published-v1");
    assert_eq!(info.info().reference.as_ref().expect("source").properties.len(), 1);
    assert_eq!(psi_dist_info(lib.resolve("Log", None)?.info(), None)?, "PsiLogNorm2");
    assert!(psi_dist_info(info.info(), Some(2)).is_err());
    let plain = PreparedInputLibrary::new(
        &library(vec![record("NoRef", PublishedData::Distribution { distribution: Distribution::Constant { value: 2.0 } })]),
        &CatalogLimits::default(),
        &NoControl,
    )?;
    assert!(psi_dist_info(plain.resolve("NoRef", None)?.info(), None).is_err_and(|e| e.code == "missing_reference_call"));
    let json = serde_json::to_value(&arithmetic).map_err(|e| SolverError::new("test", e.to_string()))?;
    assert_eq!(json["id"], "9007199254740993");
    Ok(())
}
#[test]
fn published_inputs_reject_unbounded_ambiguous_invalid_and_unknown_schema_records() -> Result<()> {
    let valid = record("Input", PublishedData::Sip { values: vec![1.0, 2.0] });
    assert!(PreparedInputLibrary::new(&library(vec![valid.clone(), valid.clone()]), &CatalogLimits::default(), &NoControl).is_err());
    let mut invalid = valid.clone();
    invalid.source.source_version.clear();
    assert!(PreparedInputLibrary::new(&library(vec![invalid]), &CatalogLimits::default(), &NoControl).is_err());
    let invalid = record("Bad", PublishedData::Sip { values: vec![f64::NAN] });
    assert!(PreparedInputLibrary::new(&library(vec![invalid]), &CatalogLimits::default(), &NoControl).is_err());
    let invalid = record("Bad", PublishedData::Slurp { rows: vec![vec![1.0, 2.0]], column_names: vec!["A".into(), "A".into()], default_sip: None });
    assert!(PreparedInputLibrary::new(&library(vec![invalid]), &CatalogLimits::default(), &NoControl).is_err());
    assert!(
        PreparedInputLibrary::new(&library(vec![valid.clone()]), &CatalogLimits { max_memory_bytes: 1, ..Default::default() }, &NoControl)
            .is_err_and(|e| e.code == "memory_limit")
    );
    let mut unknown = serde_json::to_value(&valid).map_err(|e| SolverError::new("test", e.to_string()))?;
    unknown["unverified_extra"] = serde_json::json!(true);
    assert!(serde_json::from_value::<PublishedInput>(unknown).is_err());
    let mut bad_reference = valid;
    bad_reference.reference = reference("PsiLogNormal", "PsiLogNorm2(1,2)");
    assert!(
        PreparedInputLibrary::new(&library(vec![bad_reference]), &CatalogLimits::default(), &NoControl)
            .is_err_and(|e| e.code == "invalid_reference_call")
    );
    Ok(())
}
#[test]
fn psifit_snapshot_valid_invalid_and_updated_data_are_explicit() -> Result<()> {
    use solvercraft::simulation::fitting::{FitFamily, FitOptions};
    let options = FitOptions::default();
    let first = psi_fit_snapshot(&[1.0, 2.0, 3.0], &FitFamily::Normal, &options, &NoControl)?;
    let second = psi_fit_snapshot(&[3.0, 4.0, 5.0], &FitFamily::Normal, &options, &NoControl)?;
    let Distribution::Normal { mean: first, .. } = first.distribution else { return Err(SolverError::new("test", "Wrong fitted family")) };
    let Distribution::Normal { mean: second, .. } = second.distribution else { return Err(SolverError::new("test", "Wrong fitted family")) };
    assert!((first - 2.0).abs() < 1e-12);
    assert!((second - 4.0).abs() < 1e-12);
    assert!(psi_fit_snapshot(&[1.0], &FitFamily::Normal, &options, &NoControl).is_err());
    assert!(psi_fit_snapshot(&[1.0, f64::INFINITY], &FitFamily::Normal, &options, &NoControl).is_err());
    Ok(())
}

#[test]
fn psifit_automatic_bic_policy_selects_candidates_and_invalidates_exact_snapshot_cache() -> Result<()> {
    let data = [-2.0, -1.0, 0.0, 1.0, 2.0];
    let fresh = psi_fit(&data, &NoControl)?;
    assert_eq!(fresh.policy_version, AUTOMATIC_FIT_POLICY);
    assert_eq!(fresh.candidate_families, ["normal", "uniform", "exponential", "lognormal", "laplace", "cauchy"]);
    assert!(!fresh.cache_hit);
    let Distribution::Uniform { low, high } = fresh.selected.distribution else {
        return Err(SolverError::new("test", "Independent symmetric finite-range fixture should select uniform by BIC"));
    };
    assert_eq!((low, high), (-2.0, 2.0));
    let expected_bic = 10.0 * 4.0_f64.ln() + 2.0 * 5.0_f64.ln();
    assert!((fresh.selected.bic - expected_bic).abs() < 1e-12);
    assert!(fresh.ranking.fits.iter().all(|(_, candidate)| candidate.bic >= fresh.selected.bic));
    assert!(fresh.ranking.failures.iter().any(|f| f.index == 2)); // Exponential cannot fit negative data.
    assert!(fresh.ranking.failures.iter().any(|f| f.index == 3)); // Neither can a lognormal.

    let mut cache = PsiFitCache::default();
    assert!(!cache.fit(&data, &NoControl)?.cache_hit);
    assert!(cache.fit(&data, &NoControl)?.cache_hit);
    let changed = [8.0, 9.0, 10.0, 11.0, 12.0];
    let changed_fit = cache.fit(&changed, &NoControl)?;
    assert!(!changed_fit.cache_hit);
    assert!(matches!(changed_fit.selected.distribution, Distribution::Uniform { low: 8.0, high: 12.0 }));
    assert!(cache.fit(&[f64::NAN, 1.0], &NoControl).is_err_and(|e| e.code == "fit_domain"));
    assert!(cache.fit(&changed, &NoControl)?.cache_hit); // Failed replacement preserved the successful snapshot.
    let reordered = [12.0, 11.0, 10.0, 9.0, 8.0];
    assert!(!cache.fit(&reordered, &NoControl)?.cache_hit);
    assert!(psi_fit(&[1.0], &NoControl).is_err());
    let mut limited = PsiFitCache::new(solvercraft::simulation::fitting::FitOptions { max_work: 1, ..Default::default() })?;
    assert!(limited.fit(&data, &NoControl).is_err_and(|e| e.code == "work_limit"));
    let mut limited = PsiFitCache::new(solvercraft::simulation::fitting::FitOptions { max_memory_bytes: 1, ..Default::default() })?;
    assert!(limited.fit(&data, &NoControl).is_err_and(|e| e.code == "memory_limit"));
    Ok(())
}

#[test]
fn event_inputs_honor_pre_cancel_zero_budgets_strict_schema_and_full_width_addresses() -> Result<()> {
    struct Cancel;
    impl RunControl for Cancel {
        fn is_cancelled(&self) -> bool {
            true
        }
        fn progress(&self, _: &ProgressEvent) {}
    }
    let mut request = make_request();
    // Deliberately invalid binding must never be prepared after an early stop.
    let binding = EventBinding { id: u64::MAX, distribution: Distribution::Normal { mean: 0.0, std_dev: -1.0 } };
    let result = psi_make_input(&request, std::slice::from_ref(&binding), &Cancel, |_| Ok(1.0), |_| Ok(3.0))?;
    assert_eq!(result.termination, Termination::Cancelled);
    assert_eq!(result.evaluations, 0);
    request.limits.max_evaluations = 0;
    let result = psi_make_input(&request, std::slice::from_ref(&binding), &NoControl, |_| Ok(1.0), |_| Ok(3.0))?;
    assert_eq!(result.termination, Termination::EvaluationLimit);
    request.limits = MakeInputLimits { max_work: 0, ..Default::default() };
    let result = psi_make_input(&request, std::slice::from_ref(&binding), &NoControl, |_| Ok(1.0), |_| Ok(3.0))?;
    assert_eq!(result.error.as_ref().map(|e| e.code.as_str()), Some("work_limit"));
    request.limits = MakeInputLimits::default();
    let valid = EventBinding { id: u64::MAX, distribution: Distribution::Constant { value: 1.0 } };
    assert!(psi_make_input(&request, &[valid.clone(), valid.clone()], &NoControl, |_| Ok(1.0), |_| Ok(3.0)).is_err());
    let unknown = psi_make_input(&request, std::slice::from_ref(&valid), &NoControl, |_| Ok(1.0), |event| event.sample(0))?;
    assert_eq!(unknown.termination, Termination::NumericalFailure);
    assert_eq!(unknown.completed_events, 0);
    let mut json = serde_json::to_value(&request).map_err(|e| SolverError::new("test", e.to_string()))?;
    assert_eq!(json["address"]["seed"], u64::MAX.to_string());
    assert_eq!(json["address"]["binding"], "9007199254740993");
    let restored: MakeInputRequest = serde_json::from_value(json.clone()).map_err(|e| SolverError::new("test", e.to_string()))?;
    assert_eq!(restored.address.expect("address").seed, u64::MAX);
    json["undocumented_limit_order"] = serde_json::json!(true);
    assert!(serde_json::from_value::<MakeInputRequest>(json).is_err());
    assert!(psi_fit(&[1.0, 2.0], &Cancel).is_err_and(|e| e.code == "cancelled"));
    Ok(())
}
