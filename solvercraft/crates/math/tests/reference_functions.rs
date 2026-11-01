#![allow(clippy::unwrap_used)]
use solvercraft_math::{
    cdf, quantile,
    reference_functions::{self as rf, AltOptions, Claim as C, LocationScaleFamily as L, ScaleFamily as S, ShapeFamily as F},
};
use solvercraft_types::{Distribution as D, NoControl, RunControl};
fn close(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t * (1.0 + b.abs()), "actual{a}, expected{b}");
}
fn q(p: f64, x: f64) -> C {
    C::Percentile { probability: p, value: x }
}
fn check(d: &D, claims: &[(f64, f64)]) {
    for &(p, x) in claims {
        close(cdf(d, x).unwrap(), p, 4e-7);
    }
}
#[test]
fn psi_cauchy_alt_two_percentiles_preserve_independent_tangent_anchors() {
    let d = rf::location_scale(L::Cauchy, q(0.25, -1.0), q(0.75, 5.0)).unwrap();
    check(&d, &[(0.25, -1.0), (0.75, 5.0)]);
    close(quantile(&d, 0.5).unwrap(), 2.0, 1e-13);
}
#[test]
fn psi_normal_alt_supports_both_mixed_parameter_forms() {
    let x = 2.0 + 3.0 * 1.281_551_565_544_600_4;
    for d in [
        rf::location_scale(L::Normal, q(0.9, x), C::Mean { value: 2.0 }).unwrap(),
        rf::location_scale(L::Normal, q(0.9, x), C::StdDev { value: 3.0 }).unwrap(),
    ] {
        check(&d, &[(0.9, x)]);
        close(quantile(&d, 0.5).unwrap(), 2.0, 1e-7);
    }
    assert!(rf::location_scale(L::Normal, q(0.5, 2.0), C::Mean { value: 2.0 }).is_err());
}
#[test]
fn psi_uniform_alt_endpoint_and_percentile_forms_are_distinct() {
    for d in [
        rf::location_scale(L::Uniform, C::Lower { value: -2.0 }, q(0.8, 6.0)).unwrap(),
        rf::location_scale(L::Uniform, q(0.2, 0.0), C::Upper { value: 8.0 }).unwrap(),
    ] {
        close(quantile(&d, 0.0).unwrap(), -2.0, 1e-13);
        close(quantile(&d, 1.0).unwrap(), 8.0, 1e-13);
    }
}
#[test]
fn psi_hyp_secant_alt_uses_the_documented_location_scale_convention() {
    let x = 2.0 - 3.0 * 1.0_f64.asinh();
    for d in [
        rf::location_scale(L::HyperbolicSecant, q(0.25, x), C::Location { value: 2.0 }).unwrap(),
        rf::location_scale(L::HyperbolicSecant, q(0.25, x), C::Scale { value: 3.0 }).unwrap(),
    ] {
        check(&d, &[(0.25, x), (0.5, 2.0)]);
    }
}
#[test]
fn psi_levy_alt_preserves_independent_normal_tail_anchor() {
    let d = rf::location_scale(L::Levy, q(0.317_310_507_862_914_1, 5.0), C::Location { value: 2.0 }).unwrap();
    check(&d, &[(0.317_310_507_862_914_1, 5.0)]);
    close(quantile(&d, 0.317_310_507_862_914_1).unwrap(), 5.0, 1e-6);
}
#[test]
fn psi_logistic_alt_mean_and_variance_determine_independent_quartiles() {
    let d = rf::location_scale(L::Logistic, C::Mean { value: 2.0 }, C::Variance { value: 3.0 * std::f64::consts::PI.powi(2) }).unwrap();
    check(&d, &[(0.25, 2.0 - 3.0 * 3.0_f64.ln()), (0.75, 2.0 + 3.0 * 3.0_f64.ln())]);
}
#[test]
fn psi_laplace_alt_percentile_and_stddev_match_exponential_tails() {
    let d = rf::location_scale(L::Laplace, q(0.25, 2.0 - 3.0 * 2.0_f64.ln()), C::StdDev { value: 3.0 * 2.0_f64.sqrt() }).unwrap();
    check(&d, &[(0.5, 2.0), (0.75, 2.0 + 3.0 * 2.0_f64.ln())]);
}
#[test]
fn psi_max_extreme_alt_mean_mode_and_variance_use_gumbel_constants() {
    let d = rf::location_scale(L::MaxExtreme, C::Mean { value: 2.0 + 3.0 * 0.577_215_664_901_532_9 }, C::Mode { value: 2.0 }).unwrap();
    check(&d, &[((-1.0_f64).exp(), 2.0)]);
}
#[test]
fn psi_min_extreme_alt_mean_and_scale_have_the_correct_reflection() {
    let d = rf::location_scale(L::MinExtreme, C::Mean { value: 2.0 - 3.0 * 0.577_215_664_901_532_9 }, C::Scale { value: 3.0 }).unwrap();
    check(&d, &[(1.0 - (-1.0_f64).exp(), 2.0)]);
}
#[test]
fn psi_erf_alt_variance_and_percentile_are_not_exponential_scale() {
    for d in [rf::scale_only(S::Erf, C::Variance { value: 4.0 }).unwrap(), rf::scale_only(S::Erf, q(0.841_344_746_068_542_9, 2.0)).unwrap()] {
        check(&d, &[(0.841_344_746_068_542_9, 2.0)]);
    }
    assert!(rf::scale_only(S::Erf, q(0.5, 0.0)).is_err());
}
#[test]
fn psi_exponential_alt_variance_and_percentile_preserve_mean_convention() {
    for d in
        [rf::scale_only(S::Exponential, C::Variance { value: 9.0 }).unwrap(), rf::scale_only(S::Exponential, q(0.75, 3.0 * 4.0_f64.ln())).unwrap()]
    {
        check(&d, &[(1.0 - (-1.0_f64).exp(), 3.0)]);
    }
}
#[test]
fn psi_rayleigh_alt_all_three_claims_use_sigma_not_weibull_scale() {
    for claim in [
        C::Mean { value: 3.0 * (std::f64::consts::PI / 2.0).sqrt() },
        C::Variance { value: 9.0 * (4.0 - std::f64::consts::PI) / 2.0 },
        q(1.0 - (-0.5_f64).exp(), 3.0),
    ] {
        let d = rf::scale_only(S::Rayleigh, claim).unwrap();
        check(&d, &[(1.0 - (-0.5_f64).exp(), 3.0)]);
    }
}
#[test]
fn psi_chi_square_alt_variance_and_percentile_match_df_two_exponential() {
    for d in [
        rf::shape_only_default(F::ChiSquared, C::Variance { value: 4.0 }).unwrap(),
        rf::shape_only_default(F::ChiSquared, q(0.75, 2.0 * 4.0_f64.ln())).unwrap(),
    ] {
        check(&d, &[(0.75, 2.0 * 4.0_f64.ln())]);
    }
}
#[test]
fn psi_student_alt_percentile_recovers_cauchy_df_one_and_rejects_median() {
    let d = rf::shape_only_default(F::StudentT, q(0.75, 1.0)).unwrap();
    check(&d, &[(0.75, 1.0)]);
    close(quantile(&d, 0.9).unwrap(), (0.4 * std::f64::consts::PI).tan(), 2e-6);
    assert!(rf::shape_only_default(F::StudentT, q(0.5, 0.0)).is_err());
}
#[test]
fn psi_normal_skew_is_myerson_percentile_geometry_not_azzalini_shape() {
    let tail = 0.002_699_796_146_511 / 2.0;
    for skew in [-0.5, 0.0, 0.5] {
        let d = rf::normal_skew(2.0, 14.0, skew).unwrap();
        check(&d, &[(tail, 2.0), (0.5, 2.0 + (skew + 1.0) * 6.0), (1.0 - tail, 14.0)]);
    }
    let d = rf::normal_skew(2.0, 14.0, 0.0).unwrap();
    close(quantile(&d, 0.5).unwrap(), 8.0, 1e-13);
    close(quantile(&d, 0.841_344_746_068_542_9).unwrap(), 10.0, 2e-7);
}
#[test]
fn shifted_reference_constructors_preserve_native_order_and_bounds() {
    for (d, x, p) in [
        (rf::burr12(2.0, 4.0, 2.0, 3.0).unwrap(), 6.0, 0.875),
        (rf::dagum(2.0, 4.0, 2.0, 3.0).unwrap(), 6.0, 0.125),
        (rf::fatigue_life(2.0, 4.0, 0.5).unwrap(), 6.0, 0.5),
        (rf::frechet(2.0, 4.0, 2.0).unwrap(), 6.0, (-1.0_f64).exp()),
        (rf::johnson_sb(0.0, 1.0, 2.0, 8.0).unwrap(), 5.0, 0.5),
        (rf::kumaraswamy(2.0, 3.0, 2.0, 8.0).unwrap(), 5.0, 0.578125),
        (rf::log_logistic(2.0, 4.0, 2.0).unwrap(), 6.0, 0.5),
        (rf::pareto(2.0, 3.0).unwrap(), 6.0, 0.75),
    ] {
        check(&d, &[(p, x)]);
    }
    assert!(rf::kumaraswamy(2.0, 3.0, 8.0, 2.0).is_err());
}
struct Cancel;
impl RunControl for Cancel {
    fn is_cancelled(&self) -> bool {
        true
    }
}
#[test]
fn malformed_alt_claims_do_not_return_fabricated_distributions() {
    assert!(rf::location_scale(L::Cauchy, q(0.2, 2.0), q(0.2, 3.0)).is_err());
    assert!(rf::location_scale(L::Normal, q(0.1, 3.0), q(0.9, 2.0)).is_err());
    assert!(rf::location_scale(L::Levy, C::Mean { value: 1.0 }, q(0.5, 1.0)).is_err());
    assert!(rf::shape_only(F::StudentT, q(0.75, 1.0), &AltOptions { max_evaluations: 0, ..AltOptions::default() }, &NoControl).is_err());
    assert!(rf::shape_only(F::StudentT, q(0.75, 1.0), &AltOptions::default(), &Cancel).is_err());
    assert!(rf::scale_only(S::Exponential, q(1.0, 2.0)).is_err());
}
#[test]
fn psi_gamma_alt_supports_two_quantiles_and_all_native_moment_pairs() {
    use rf::ShapeScaleFamily as G;
    for (a, b) in [
        (q(0.25, -3.0 * 0.75_f64.ln()), q(0.75, 3.0 * 4.0_f64.ln())),
        (C::Mean { value: 3.0 }, C::Variance { value: 9.0 }),
        (C::Shape { value: 1.0 }, q(0.75, 3.0 * 4.0_f64.ln())),
        (C::Scale { value: 3.0 }, q(0.75, 3.0 * 4.0_f64.ln())),
        (C::Shape { value: 1.0 }, C::Variance { value: 9.0 }),
        (C::Mean { value: 3.0 }, C::Scale { value: 3.0 }),
    ] {
        let d = rf::shape_scale_default(G::Gamma, a, b).unwrap();
        check(&d, &[(0.75, 3.0 * 4.0_f64.ln()), (1.0 - (-1.0_f64).exp(), 3.0)]);
    }
}
#[test]
fn psi_pearson5_alt_recovers_inverse_exponential_quantiles() {
    let d = rf::shape_scale_default(rf::ShapeScaleFamily::InverseGamma, q(0.25, -2.0 / 0.25_f64.ln()), q(0.75, -2.0 / 0.75_f64.ln())).unwrap();
    check(&d, &[(0.5, 2.0 / 2.0_f64.ln()), ((-1.0_f64).exp(), 2.0)]);
}
#[test]
fn psi_pareto_alt_recovers_independent_power_quantiles_and_native_scale() {
    let d = rf::shape_scale_default(rf::ShapeScaleFamily::Pareto, q(0.25, 3.0 / 0.75_f64.sqrt()), q(0.75, 6.0)).unwrap();
    check(&d, &[(0.0, 3.0), (0.75, 6.0)]);
    let d = rf::shape_scale_default(rf::ShapeScaleFamily::Pareto, C::Scale { value: 3.0 }, q(0.75, 6.0)).unwrap();
    check(&d, &[(0.75, 6.0)]);
}
#[test]
fn psi_pareto2_alt_recovers_lomax_quantiles_and_native_shape() {
    let d = rf::shape_scale_default(rf::ShapeScaleFamily::Pareto2, q(0.25, 3.0 * (1.0 / 0.75_f64.sqrt() - 1.0)), q(0.75, 3.0)).unwrap();
    check(&d, &[(0.75, 3.0)]);
    let d = rf::shape_scale_default(rf::ShapeScaleFamily::Pareto2, C::Shape { value: 2.0 }, q(0.75, 3.0)).unwrap();
    check(&d, &[(0.75, 3.0)]);
}
#[test]
fn psi_cauchy_native_parameter_convention_has_independent_quartiles() {
    check(&D::Cauchy { location: 2.0, scale: 3.0 }, &[(0.25, -1.0), (0.5, 2.0), (0.75, 5.0)]);
}
#[test]
fn psi_inv_normal_native_mean_shape_have_independent_gaussian_anchor() {
    check(&D::InverseGaussian { mean: 1.0, shape: 1.0 }, &[(0.668_102_001_223_170_6, 1.0)]);
}
#[test]
fn psi_johnson_su_native_shapes_location_scale_have_independent_normal_anchor() {
    check(&D::JohnsonSu { gamma: 0.0, delta: 1.0, location: 2.0, scale: 3.0 }, &[(0.841_344_746_068_542_9, 2.0 + 3.0 * 1.0_f64.sinh())]);
}
#[test]
fn psi_laplace_native_location_scale_have_independent_tail_anchors() {
    check(&D::Laplace { location: 2.0, scale: 3.0 }, &[(0.25, 2.0 - 3.0 * 2.0_f64.ln()), (0.75, 2.0 + 3.0 * 2.0_f64.ln())]);
}
#[test]
fn psi_max_extreme_native_location_scale_have_independent_exponential_anchor() {
    check(&D::Gumbel { location: 2.0, scale: 3.0 }, &[((-1.0_f64).exp(), 2.0)]);
}
#[test]
fn psi_pearson5_native_shape_scale_have_independent_inverse_exponential_anchor() {
    check(&D::InverseGamma { shape: 1.0, scale: 2.0 }, &[((-1.0_f64).exp(), 2.0)]);
}
#[test]
fn psi_rayleigh_native_sigma_has_independent_chisquare_anchor() {
    check(&D::Rayleigh { scale: 3.0 }, &[(1.0 - (-0.5_f64).exp(), 3.0)]);
}
fn three_alt(family: rf::LocationShapeScaleFamily, values: [f64; 3], probabilities: [f64; 3], location: f64, scale: f64, shape: f64) {
    let points = [q(probabilities[0], values[0]), q(probabilities[1], values[1]), q(probabilities[2], values[2])];
    for claims in [
        points.to_vec(),
        vec![points[0], points[2], C::Location { value: location }],
        vec![points[0], points[2], C::Scale { value: scale }],
        vec![points[0], points[2], C::Shape { value: shape }],
        vec![points[0], C::Location { value: location }, C::Scale { value: scale }],
        vec![points[0], C::Location { value: location }, C::Shape { value: shape }],
        vec![points[0], C::Scale { value: scale }, C::Shape { value: shape }],
    ] {
        let d = rf::location_shape_scale_default(family, &claims).unwrap();
        check(&d, &[(probabilities[0], values[0]), (probabilities[1], values[1]), (probabilities[2], values[2])]);
    }
}
#[test]
fn psi_fatigue_life_alt_all_percentile_native_combinations_preserve_gaussian_geometry() {
    let ps = [0.158_655_253_931_457_07, 0.5, 0.841_344_746_068_542_9];
    let xs = [1.0 + 2.0 * (-0.25 + 1.0625_f64.sqrt()).powi(2), 3.0, 1.0 + 2.0 * (0.25 + 1.0625_f64.sqrt()).powi(2)];
    three_alt(rf::LocationShapeScaleFamily::FatigueLife, xs, ps, 1.0, 2.0, 0.5);
}
#[test]
fn psi_frechet_alt_all_percentile_native_combinations_preserve_power_exponential_geometry() {
    let ps = [0.25_f64, 0.5, 0.75];
    let xs = ps.map(|p| 1.0 + 3.0 / (-p.ln()).sqrt());
    three_alt(rf::LocationShapeScaleFamily::Frechet, xs, ps, 1.0, 3.0, 2.0);
}
#[test]
fn psi_log_logistic_alt_three_parameters_preserve_nonzero_location() {
    let ps = [0.25_f64, 0.5, 0.75];
    let xs = ps.map(|p| 1.0 + 3.0 * (p / (1.0 - p)).sqrt());
    three_alt(rf::LocationShapeScaleFamily::LogLogistic, xs, ps, 1.0, 3.0, 2.0);
}
#[test]
fn psi_inv_normal_alt_mean_variance_and_native_lambda_have_independent_cdf_anchors() {
    let p = 0.668_102_001_223_170_6;
    for (a, b) in [
        (C::Mean { value: 1.0 }, C::Variance { value: 1.0 }),
        (C::Mean { value: 1.0 }, C::Scale { value: 1.0 }),
        (C::Variance { value: 1.0 }, C::Scale { value: 1.0 }),
        (C::Mean { value: 1.0 }, q(p, 1.0)),
        (C::Variance { value: 1.0 }, q(p, 1.0)),
        (C::Scale { value: 1.0 }, q(p, 1.0)),
    ] {
        let d = rf::inverse_gaussian(a, b, &AltOptions::default(), &NoControl).unwrap();
        check(&d, &[(p, 1.0)]);
    }
}
#[test]
fn psi_inv_normal_alt_two_quantiles_match_independent_scipy_values() {
    let d = rf::inverse_gaussian(q(0.364_975_548_172_959_96, 0.5), q(0.885_475_425_986_006_5, 2.0), &AltOptions::default(), &NoControl).unwrap();
    check(&d, &[(0.668_102_001_223_170_6, 1.0), (0.979_076_364_178_886_2, 4.0)]);
}
#[test]
fn psi_log_normal_alt_mixed_arithmetic_claims_do_not_confuse_log_parameters() {
    let mean = (1.0 + 0.5 * 0.5 * 0.5_f64).exp();
    let std = mean * 0.25_f64.exp_m1().sqrt();
    for c in [C::Mean { value: mean }, C::StdDev { value: std }] {
        let d = rf::lognormal(q(0.5, 1.0_f64.exp()), c, &AltOptions::default(), &NoControl).unwrap();
        check(&d, &[(0.158_655_253_931_457_07, 0.5_f64.exp()), (0.841_344_746_068_542_9, 1.5_f64.exp())]);
    }
    assert_eq!(
        rf::lognormal(q(0.841_344_746_068_542_9, 1.0), C::Mean { value: 0.9 }, &AltOptions::default(), &NoControl).unwrap_err().code,
        "alt_ambiguous"
    );
}
#[test]
fn psi_beta_gen_alt_four_percentiles_recover_independent_beta_polynomial() {
    let f = |x: f64| 6.0 * x * x - 8.0 * x * x * x + 3.0 * x.powi(4);
    let claims = [q(f(0.2), 0.0), q(f(0.4), 2.0), q(f(0.6), 4.0), q(f(0.8), 6.0)];
    let options = AltOptions { max_evaluations: 4096, ..AltOptions::default() };
    let d = rf::bounded(rf::BoundedFamily::BetaGeneral, &claims, &options, &NoControl).unwrap();
    check(&d, &[(f(0.3), 1.0), (f(0.5), 3.0), (f(0.7), 5.0)]);
    let d = rf::bounded(
        rf::BoundedFamily::BetaGeneral,
        &[C::Shape1 { value: 2.0 }, C::Shape2 { value: 3.0 }, C::Lower { value: -2.0 }, C::Upper { value: 8.0 }],
        &options,
        &NoControl,
    )
    .unwrap();
    check(&d, &[(f(0.5), 3.0)]);
}
#[test]
fn psi_pert_alt_three_percentiles_and_native_moments_preserve_weight_four() {
    // Unit mean/mode .25: alpha2,beta4, exact CDF10x²-20x³+15x⁴-4x⁵.
    let f = |x: f64| 10.0 * x * x - 20.0 * x.powi(3) + 15.0 * x.powi(4) - 4.0 * x.powi(5);
    let options = AltOptions { max_evaluations: 4096, ..AltOptions::default() };
    let d = rf::bounded(rf::BoundedFamily::Pert, &[q(f(0.2), 0.2), q(f(0.5), 0.5), q(f(0.8), 0.8)], &options, &NoControl).unwrap();
    check(&d, &[(f(0.3), 0.3), (f(0.7), 0.7)]);
    let d = rf::bounded(rf::BoundedFamily::Pert, &[C::Lower { value: 0.0 }, C::Mode { value: 0.25 }, C::Upper { value: 1.0 }], &options, &NoControl)
        .unwrap();
    check(&d, &[(f(0.3), 0.3), (f(0.7), 0.7)]);
}
#[test]
fn psi_triangular_alt_three_percentiles_cover_both_piecewise_branches() {
    let options = AltOptions { max_evaluations: 4096, ..AltOptions::default() };
    let d = rf::bounded(rf::BoundedFamily::Triangular, &[q(0.16, 0.2), q(2.0 / 3.0, 0.5), q(1.0 - 0.04 / 0.75, 0.8)], &options, &NoControl).unwrap();
    check(&d, &[(0.25, 0.25), (1.0 - 0.25_f64.powi(2) / 0.75, 0.75)]);
    // Three points on only the lower quadratic branch cannot identify all3 parameters.
    assert!(rf::bounded(rf::BoundedFamily::Triangular, &[q(0.04, 0.1), q(0.16, 0.2), q(0.36, 0.3)], &options, &NoControl).is_err());
}
#[test]
fn psi_pearson6_alt_mathematical_three_claim_core_matches_beta_prime_polynomial() {
    let f = |x: f64| {
        let u = x / (3.0 + x);
        6.0 * u * u - 8.0 * u.powi(3) + 3.0 * u.powi(4)
    };
    let options = AltOptions { max_evaluations: 4096, ..AltOptions::default() };
    let d = rf::bounded(rf::BoundedFamily::BetaPrime, &[q(f(1.0), 1.0), q(f(3.0), 3.0), q(f(9.0), 9.0)], &options, &NoControl).unwrap();
    check(&d, &[(f(2.0), 2.0), (f(5.0), 5.0)]);
    let d = rf::bounded(
        rf::BoundedFamily::BetaPrime,
        &[C::Shape1 { value: 2.0 }, C::Shape2 { value: 3.0 }, C::Scale { value: 3.0 }],
        &options,
        &NoControl,
    )
    .unwrap();
    check(&d, &[(f(2.0), 2.0), (f(5.0), 5.0)]);
    // Printed two-claim Alt arity cannot identify three unrestricted parameters.
    assert!(rf::bounded(rf::BoundedFamily::BetaPrime, &[q(f(1.0), 1.0), q(f(9.0), 9.0)], &options, &NoControl).is_err());
}
#[test]
fn psi_weibull_alt_mathematical_two_claim_core_preserves_power_exponential_cdf() {
    let x1 = 3.0 * (-0.75_f64.ln()).sqrt();
    let x2 = 3.0 * 4.0_f64.ln().sqrt();
    for (a, b) in [(q(0.25, x1), q(0.75, x2)), (C::Shape { value: 2.0 }, q(0.75, x2)), (C::Scale { value: 3.0 }, q(0.75, x2))] {
        let d = rf::weibull(a, b).unwrap();
        check(&d, &[(1.0 - (-1.0_f64).exp(), 3.0)]);
    }
}
#[test]
fn inverse_parameter_contracts_reject_unknown_fields_and_enforce_budgets() {
    use serde_json::json;
    for fixture in [
        json!({"kind":"percentile","probability":0.5,"value":2}),
        json!({"kind":"mean","value":2}),
        json!({"kind":"variance","value":2}),
        json!({"kind":"std_dev","value":2}),
        json!({"kind":"location","value":2}),
        json!({"kind":"scale","value":2}),
        json!({"kind":"shape","value":2}),
        json!({"kind":"shape1","value":2}),
        json!({"kind":"shape2","value":2}),
        json!({"kind":"lower","value":2}),
        json!({"kind":"upper","value":2}),
        json!({"kind":"mode","value":2}),
    ] {
        let _: C = serde_json::from_value(fixture.clone()).unwrap();
        let mut extra = fixture;
        extra.as_object_mut().unwrap().insert("unknown".into(), json!(true));
        assert!(serde_json::from_value::<C>(extra).is_err());
    }
    let claims = [C::Lower { value: 0.0 }, C::Mode { value: 0.25 }, C::Upper { value: 1.0 }];
    assert_eq!(
        rf::bounded(rf::BoundedFamily::Pert, &claims, &AltOptions { max_evaluations: 0, ..AltOptions::default() }, &NoControl).unwrap_err().code,
        "evaluation_limit"
    );
    assert_eq!(rf::bounded(rf::BoundedFamily::Pert, &claims, &AltOptions::default(), &Cancel).unwrap_err().code, "cancelled");
    // A large location must not hide a completely wrong positive variance in
    // the numerical fit's normalized residual or rounded returned parameters.
    assert!(
        rf::bounded(
            rf::BoundedFamily::Pert,
            &[C::Mean { value: 1e16 }, C::Mode { value: 1e16 }, C::Variance { value: 0.01 }],
            &AltOptions { max_evaluations: 4096, ..AltOptions::default() },
            &NoControl
        )
        .is_err()
    );
}
