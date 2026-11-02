//! Frozen, inspectable family catalog. Compositional modifiers are separate entries.
use serde::Serialize;
pub const CATALOG_VERSION: &str = "solvercraft-baseline17-r4-catalog-v3";
#[derive(Debug, Clone, Copy, Serialize)]
pub struct DistributionDescriptor {
    pub id: &'static str,
    pub baseline: bool,
    pub discrete: bool,
    pub inverse_cdf: bool,
    pub note: &'static str,
}
macro_rules! entry {
    ($id:literal,$base:expr,$disc:expr,$note:literal) => {
        DistributionDescriptor { id: $id, baseline: $base, discrete: $disc, inverse_cdf: true, note: $note }
    };
}
pub const DISTRIBUTIONS: &[DistributionDescriptor] = &[
    entry!("metalog", false, false, "Original2016 basis2..30terms; globalDz-dominance certificate required; upper-only bounds unsupported."),
    entry!("burr12", false, false, "Positive scale, shapes1e-4..1e4."),
    entry!("dagum", false, false, "Positive scale, shapes1e-4..1e4."),
    entry!("double_triangular", false, false, "Independent left branch mass, strict mode interior."),
    entry!("fatigue_life", false, false, "Birnbaum-Saunders on positive support; use shift for location."),
    entry!("frechet", false, false, "Positive scale/shape; shift for location."),
    entry!("hyperbolic_secant", false, false, "CDF2/pi atan(exp((x-location)/scale))."),
    entry!("kumaraswamy", false, false, "Unit interval, positive shapes."),
    entry!("levy", false, false, "Location/scale Levy with right tail."),
    entry!("pareto2", false, false, "Lomax on nonnegative support."),
    entry!("beta_prime", false, false, "Scaled beta-prime/PearsonVI."),
    entry!("reciprocal", false, false, "Log-uniform between positive finite bounds."),
    entry!("min_extreme", false, false, "Minimum extreme-value convention."),
    entry!("logarithmic", false, true, "Log-series support1..; p<=0.99, inverse count cap100000."),
    entry!("constant", false, true, "Point mass."),
    entry!("uniform", true, false, "Finite endpoints."),
    entry!("discrete_uniform", true, true, "Integer endpoints have magnitude <2^53."),
    entry!("normal", true, false, "CDF uses direct regularized-gamma upper tails; addressed normal inverse draws unchanged."),
    entry!("lognormal", true, false, "Normal parameters are on the log scale."),
    entry!("triangular", true, false, "Finite low <= mode <= high."),
    entry!("beta_pert", true, false, "Weighted Beta-PERT on finite bounds."),
    entry!("beta", true, false, "Shapes 1e-4..1e4."),
    entry!("gamma", true, false, "Shape 1e-4..1e4, positive scale."),
    entry!("exponential", true, false, "Positive rate."),
    entry!("weibull", true, false, "Positive shape and scale."),
    entry!("bernoulli", true, true, "Probability in [0,1]."),
    entry!("binomial", true, true, "At most 10000 nondegenerate trials."),
    entry!("poisson", true, true, "Rate at most 512."),
    entry!("negative_binomial", true, true, "Failures before r successes; r1..10000; quantile <=1e9."),
    entry!("geometric", true, true, "Failures before first success, support 0,1,..."),
    entry!("empirical", true, true, "Equal weights, at most 1million values."),
    entry!("empirical_continuous", true, false, "Piecewise linear quantile; duplicate knots can create atoms."),
    entry!("student_t", false, false, "Location/scale t, df1e-4..1e4."),
    entry!("chi_squared", false, false, "df1e-4..1e4."),
    entry!("f", false, false, "Both degrees of freedom1e-4..1e4."),
    entry!("cauchy", false, false, "Location and positive scale."),
    entry!("logistic", false, false, "Location and positive scale."),
    entry!("laplace", false, false, "Location and positive scale."),
    entry!("gumbel", false, false, "Maximum extreme-value convention."),
    entry!("pareto", false, false, "Type I, positive minimum and shape."),
    entry!("rayleigh", false, false, "Positive scale."),
    entry!("erlang", false, false, "Integer shape1..10000."),
    entry!("inverse_gamma", false, false, "Positive scale; shape1e-4..1e4."),
    entry!("inverse_gaussian", false, false, "Positive mean and shape."),
    entry!("log_logistic", false, false, "Positive scale; shape1e-4..1e4."),
    entry!("skew_normal", false, false, "Shape magnitude <=100; bounded adaptive positive-tail quadrature."),
    entry!("johnson_su", false, false, "Unbounded Johnson branch."),
    entry!("johnson_sb", false, false, "Bounded Johnson branch."),
    entry!("johnson_sl", false, false, "Shifted lognormal Johnson branch; SN is Normal."),
    entry!("hypergeometric", false, true, "Population<=1million, draws<=10000."),
    entry!("categorical", false, true, "At most100000 weighted finite values."),
    entry!("shift_scale", false, false, "Finite nonzero scale; negative reflection supported."),
    entry!("truncated", false, false, "Inclusive bounds; retained CDF mass >=1e-9."),
    entry!("mixture", false, false, "At most32 components, common scalar CDF; generic inverse numerical."),
    DistributionDescriptor {
        id: "compound_poisson",
        baseline: false,
        discrete: false,
        inverse_cdf: false,
        note: "MC-only severity sum; rate<=512, event cap<=10000, no nested compounds.",
    },
];
pub fn distribution_catalog() -> &'static [DistributionDescriptor] {
    DISTRIBUTIONS
}
