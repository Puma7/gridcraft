//! Caller-published uncertain inputs and general event-expression compounding.
//!
//! Reference: frozen 2025 Q1 pages 295–296/334–335 and official PsiMakeInput help.
//! No proprietary catalog, external fetch, file evaluation or certification is implied.
use crate::{math, simulation, types::*};
use serde::{Deserialize, Serialize};
use simulation::resampling::{PreparedTable, PreparedValues, ResamplingLimits, VectorAddress};
use std::io::Write;
pub const INPUT_SCHEMA_VERSION: u32 = 1;
pub const EVENT_STREAM_VERSION: &str = "solvercraft-make-input-v1/addressed-mix64-v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationSource {
    pub publisher: String,
    pub source_id: String,
    pub source_version: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceCall {
    /// Preserved publisher-supplied reference identity; never inferred from a DTO family.
    pub name: String,
    pub formula: String,
    pub source_version: String,
    /// Informational source properties, not a claim that arbitrary properties execute.
    #[serde(default)]
    pub properties: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PublishedData {
    Distribution {
        distribution: Distribution,
    },
    Sip {
        values: Vec<f64>,
    },
    Slurp {
        rows: Vec<Vec<f64>>,
        column_names: Vec<String>,
        #[serde(default)]
        default_sip: Option<String>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishedInput {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub source: PublicationSource,
    #[serde(default)]
    pub reference: Option<ReferenceCall>,
    #[serde(default)]
    pub notes: Vec<String>,
    pub data: PublishedData,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputLibrary {
    pub schema_version: u32,
    pub records: Vec<PublishedInput>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CatalogLimits {
    pub max_records: usize,
    pub max_cells: usize,
    pub max_memory_bytes: u64,
}
impl Default for CatalogLimits {
    fn default() -> Self {
        Self { max_records: 1024, max_cells: 1_000_000, max_memory_bytes: 64 * 1024 * 1024 }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputInfo {
    pub id: String,
    pub name: String,
    pub source: PublicationSource,
    pub reference: Option<ReferenceCall>,
    pub notes: Vec<String>,
}
enum PreparedData {
    Distribution(math::PreparedDistribution),
    Sip(PreparedValues),
    Slurp { table: PreparedTable, columns: Vec<String>, default_column: Option<usize> },
}
struct PreparedInput {
    info: InputInfo,
    data: PreparedData,
}
pub struct PreparedInputLibrary {
    records: Vec<PreparedInput>,
}
pub struct ResolvedInput<'a> {
    input: &'a PreparedInput,
    column: Option<usize>,
}
fn error(code: &str, message: &str) -> SolverError {
    SolverError::new(code, message)
}
fn text(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && value.trim() == value && !value.chars().any(char::is_control)
}
fn cancelled(control: &dyn RunControl) -> Result<()> {
    if control.is_cancelled() { Err(error("cancelled", "Input operation cancelled.")) } else { Ok(()) }
}
fn reserve<T>(out: &mut Vec<T>, count: usize) -> Result<()> {
    out.try_reserve_exact(count).map_err(|_| error("memory_limit", "Cannot allocate bounded input storage."))
}
fn add_size(total: &mut u64, value: u64) -> Result<()> {
    *total = total.checked_add(value).ok_or_else(|| error("memory_limit", "Input storage estimate overflow."))?;
    Ok(())
}
fn check_depth(distribution: &Distribution, depth: u32, nodes: &mut usize) -> Result<()> {
    *nodes = nodes.checked_add(1).ok_or_else(|| error("input_limit", "Distribution node overflow."))?;
    if depth > 16 || *nodes > 100_000 {
        return Err(error("input_limit", "Published distribution nesting/node limit exceeded."));
    }
    match distribution {
        Distribution::ShiftScale { base, .. } | Distribution::Truncated { base, .. } => check_depth(base, depth + 1, nodes)?,
        Distribution::Mixture { components, .. } => {
            if components.len() > 32 {
                return Err(error("input_limit", "Mixtures allow at most32 components."));
            }
            for c in components {
                check_depth(c, depth + 1, nodes)?;
            }
        }
        Distribution::CompoundPoisson { severity, .. } => check_depth(severity, depth + 1, nodes)?,
        _ => {}
    }
    Ok(())
}
fn distribution_memory(distribution: &Distribution, limit: u64) -> Result<u64> {
    check_depth(distribution, 0, &mut 0)?;
    struct Counter {
        bytes: u64,
        limit: u64,
    }
    impl Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.bytes = self
                .bytes
                .checked_add(bytes.len() as u64)
                .filter(|n| *n <= self.limit)
                .ok_or_else(|| std::io::Error::other("Distribution byte budget"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter { bytes: 0, limit };
    serde_json::to_writer(&mut counter, distribution).map_err(|_| error("memory_limit", "Distribution representation exceeds memory budget."))?;
    let mut bytes =
        counter.bytes.checked_mul(16).and_then(|n| n.checked_add(8192)).ok_or_else(|| error("memory_limit", "Distribution size overflow."))?;
    fn tables(d: &Distribution) -> u64 {
        match d {
            Distribution::Binomial { trials, .. } => (u64::from(*trials) + 1) * 32,
            Distribution::Poisson { .. } => 262144,
            Distribution::ShiftScale { base, .. } | Distribution::Truncated { base, .. } => tables(base),
            Distribution::Mixture { components, .. } => components.iter().map(tables).sum(),
            Distribution::CompoundPoisson { severity, .. } => tables(severity),
            _ => 0,
        }
    }
    add_size(&mut bytes, tables(distribution))?;
    Ok(bytes)
}
fn metadata(record: &PublishedInput) -> Result<u64> {
    if record.schema_version != INPUT_SCHEMA_VERSION
        || !text(&record.id, 128)
        || !text(&record.name, 256)
        || !text(&record.source.publisher, 256)
        || !text(&record.source.source_id, 256)
        || !text(&record.source.source_version, 128)
        || record.notes.len() > 32
        || record.notes.iter().any(|s| !text(s, 1024))
    {
        return Err(error("invalid_publication", "Published IDs, source version and notes must be bounded canonical strings."));
    }
    let mut bytes = 4096_u64;
    for value in [&record.id, &record.name, &record.source.publisher, &record.source.source_id, &record.source.source_version]
        .into_iter()
        .chain(record.notes.iter())
    {
        add_size(&mut bytes, value.len() as u64 * 2)?;
    }
    if let Some(reference) = &record.reference {
        check_reference(reference)?;
        add_size(&mut bytes, (reference.formula.len() + reference.name.len() + reference.source_version.len()) as u64 * 2)?;
        for p in &reference.properties {
            add_size(&mut bytes, p.len() as u64 * 2)?;
        }
    }
    Ok(bytes)
}
impl PreparedInputLibrary {
    /// Prepares only caller-provided records. Publication provenance is a source
    /// claim, not an SDK-issued certification or vendor catalog verification.
    pub fn new(library: &InputLibrary, limits: &CatalogLimits, control: &dyn RunControl) -> Result<Self> {
        if library.schema_version != INPUT_SCHEMA_VERSION
            || limits.max_records > 4096
            || limits.max_cells > 1_000_000
            || library.records.len() > limits.max_records
        {
            return Err(error("input_limit", "Unsupported library schema or record/cell limit."));
        }
        cancelled(control)?;
        let mut bytes = 1024_u64;
        let mut cells = 0usize;
        for (index, record) in library.records.iter().enumerate() {
            add_size(&mut bytes, metadata(record)?)?;
            if library.records[..index].iter().any(|old| old.name == record.name || old.id == record.id) {
                return Err(error("duplicate_publication", "Published input names and IDs must be unique."));
            }
            match &record.data {
                PublishedData::Distribution { distribution } => add_size(&mut bytes, distribution_memory(distribution, limits.max_memory_bytes)?)?,
                PublishedData::Sip { values } => {
                    cells = cells.checked_add(values.len()).ok_or_else(|| error("input_limit", "Cell count overflow."))?;
                    add_size(&mut bytes, values.len() as u64 * 24 + 256)?;
                }
                PublishedData::Slurp { rows, column_names, default_sip } => {
                    if column_names.is_empty()
                        || column_names.len() > 4096
                        || column_names.iter().any(|s| !text(s, 256))
                        || column_names.iter().enumerate().any(|(index, name)| column_names[..index].contains(name))
                        || rows.iter().any(|r| r.len() != column_names.len())
                        || default_sip.as_ref().is_some_and(|name| !column_names.contains(name))
                    {
                        return Err(error("invalid_slurp", "SLURP requires rectangular data, unique columns and a valid publisher default."));
                    }
                    let count = rows.len().checked_mul(column_names.len()).ok_or_else(|| error("input_limit", "SLURP cell overflow."))?;
                    cells = cells.checked_add(count).ok_or_else(|| error("input_limit", "Cell count overflow."))?;
                    add_size(&mut bytes, count as u64 * 24 + rows.len() as u64 * 48 + column_names.len() as u64 * 512 + 256)?;
                }
            }
            if cells > limits.max_cells || bytes > limits.max_memory_bytes {
                return Err(error("memory_limit", "Published library exceeds cell/memory budget before preparation."));
            }
            cancelled(control)?;
        }
        if bytes > limits.max_memory_bytes {
            return Err(error("memory_limit", "Published library exceeds memory budget."));
        }
        let replay_limits = ResamplingLimits { max_cells: limits.max_cells, max_memory_bytes: limits.max_memory_bytes };
        let mut records = Vec::new();
        reserve(&mut records, library.records.len())?;
        for record in &library.records {
            cancelled(control)?;
            let data = match &record.data {
                PublishedData::Distribution { distribution } => PreparedData::Distribution(math::PreparedDistribution::new(distribution)?),
                PublishedData::Sip { values } => PreparedData::Sip(PreparedValues::new(values, &replay_limits, control)?),
                PublishedData::Slurp { rows, column_names, default_sip } => PreparedData::Slurp {
                    table: PreparedTable::new(rows, &replay_limits, control)?,
                    columns: column_names.clone(),
                    default_column: default_sip.as_ref().and_then(|s| column_names.iter().position(|c| c == s)),
                },
            };
            records.push(PreparedInput {
                info: InputInfo {
                    id: record.id.clone(),
                    name: record.name.clone(),
                    source: record.source.clone(),
                    reference: record.reference.clone(),
                    notes: record.notes.clone(),
                },
                data,
            });
        }
        records.sort_unstable_by(|a, b| a.info.name.cmp(&b.info.name));
        Ok(Self { records })
    }
    pub fn resolve(&self, name: &str, sip_name: Option<&str>) -> Result<ResolvedInput<'_>> {
        if !text(name, 256) || sip_name.is_some_and(|s| !text(s, 256)) {
            return Err(error("invalid_input_name", "Published and SIP names must be nonempty canonical strings."));
        }
        let index = self
            .records
            .binary_search_by(|r| r.info.name.as_str().cmp(name))
            .map_err(|_| error("unknown_input", "Published input name was not provided by the caller."))?;
        let input = &self.records[index];
        let column = match &input.data {
            PreparedData::Slurp { columns, default_column, .. } => Some(if let Some(name) = sip_name {
                columns.iter().position(|c| c == name).ok_or_else(|| error("unknown_sip", "Named SIP column is absent from the publication."))?
            } else if let Some(column) = default_column {
                *column
            } else if columns.len() == 1 {
                0
            } else {
                return Err(error("missing_default_sip", "Multi-column publication requires a selected SIP or publisher default metadata."));
            }),
            _ => {
                if sip_name.is_some() {
                    return Err(error("unexpected_sip", "SIP column selector applies only to a SLURP publication."));
                }
                None
            }
        };
        Ok(ResolvedInput { input, column })
    }
}
impl ResolvedInput<'_> {
    pub fn info(&self) -> &InputInfo {
        &self.input.info
    }
    /// Replay remains sequential by outer trial; it is not secretly bootstrapped.
    pub fn sample(&self, address: Option<VectorAddress>, control: &dyn RunControl) -> Result<f64> {
        let address = address.ok_or_else(|| error("simulation_context_required", "Published input sampling needs an explicit trial context."))?;
        cancelled(control)?;
        match &self.input.data {
            PreparedData::Distribution(d) => d.sample(address.seed, address.binding, address.trial),
            PreparedData::Sip(d) => d.sip(address.trial, control),
            PreparedData::Slurp { table, .. } => {
                table.slurp(address.trial, self.column.ok_or_else(|| error("missing_default_sip", "No SIP column selected."))? + 1, control)
            }
        }
    }
}
/// Default/0 returns the preserved distribution name;1 returns its exact source
/// formula. Missing source metadata is an error, never a guessed parameterization.
pub fn psi_dist_info(info: &InputInfo, selector: Option<u8>) -> Result<String> {
    if selector.is_some_and(|selector| selector > 1) {
        return Err(error("invalid_dist_info", "Distribution info selector must be omitted,0 or1."));
    }
    let reference =
        info.reference.as_ref().ok_or_else(|| error("missing_reference_call", "Distribution identity/formula was not supplied by the publisher."))?;
    check_reference(reference)?;
    match selector.unwrap_or(0) {
        0 => Ok(reference.name.clone()),
        1 => Ok(reference.formula.clone()),
        _ => Err(error("invalid_dist_info", "Distribution info selector must be omitted,0 or1.")),
    }
}
fn check_reference(reference: &ReferenceCall) -> Result<()> {
    if !text(&reference.name, 64)
        || !reference.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        || !text(&reference.source_version, 128)
        || !text(&reference.formula, 16384)
        || reference.properties.len() > 32
        || reference.properties.iter().any(|p| !text(p, 4096))
    {
        return Err(error("invalid_reference_call", "Reference call metadata exceeds its strict contract."));
    }
    let formula = reference.formula.strip_prefix('=').unwrap_or(&reference.formula);
    if !formula.starts_with(&format!("{}(", reference.name)) || !formula.ends_with(')') {
        return Err(error("invalid_reference_call", "Preserved formula must name its declared reference function."));
    }
    Ok(())
}

/// Explicit-family data snapshot fit. For automatic candidate selection and
/// snapshot change detection, use [`psi_fit`] or [`PsiFitCache`].
pub fn psi_fit_snapshot(
    values: &[f64],
    family: &simulation::fitting::FitFamily,
    options: &simulation::fitting::FitOptions,
    control: &dyn RunControl,
) -> Result<simulation::fitting::FitReport> {
    simulation::fitting::fit_distribution(values, family, options, control)
}

/// Solvercraft's published default, not an assertion about proprietary defaults.
pub const AUTOMATIC_FIT_POLICY: &str = "solvercraft-continuous-six-bic-v1";
const AUTOMATIC_FIT_FAMILIES: [&str; 6] = ["normal", "uniform", "exponential", "lognormal", "laplace", "cauchy"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomaticFitResult {
    pub selected: simulation::fitting::FitReport,
    pub ranking: simulation::fitting::FitRanking,
    pub candidate_families: Vec<String>,
    pub policy_version: String,
    pub cache_hit: bool,
    pub warnings: Vec<String>,
}

/// One bounded, exact-bit snapshot cache. Changing any observation (including
/// its order or signed zero) causes fitting to run again. Failed recomputations
/// leave the previous successful snapshot intact. Hosts call `fit` when their
/// linked data changes; this SDK does not watch arbitrary workbook storage.
///
/// The default candidates are continuous PDFs only: normal, uniform,
/// exponential, lognormal, Laplace and Cauchy. BIC ranks successful candidates;
/// equal scores use the displayed candidate order. Discrete/counting models are
/// deliberately not compared with continuous densities. Integer-valued input
/// alone does not identify a measurement model, so the default remains continuous.
#[derive(Default)]
pub struct PsiFitCache {
    options: simulation::fitting::FitOptions,
    snapshot: Vec<u64>,
    result: Option<AutomaticFitResult>,
}
impl PsiFitCache {
    pub fn new(options: simulation::fitting::FitOptions) -> Result<Self> {
        if options.max_evaluations == 0 || options.max_evaluations > 100_000 || options.bootstrap_replicates > 500 {
            return Err(error("fit_domain", "Automatic fitting evaluation limit is 1..100000, bootstrap limit is 500."));
        }
        Ok(Self { options, snapshot: Vec::new(), result: None })
    }

    pub fn fit(&mut self, values: &[f64], control: &dyn RunControl) -> Result<AutomaticFitResult> {
        use simulation::fitting::{FitFamily, RankingCriterion, rank_fits};
        cancelled(control)?;
        if values.len() < 2 || values.len() > 100_000 || values.iter().any(|v| !v.is_finite()) {
            return Err(error("fit_domain", "Automatic fitting requires 2..100000 finite observations."));
        }
        // Shared fitting reserves 64 bytes per observation plus 64 KiB. Include
        // both key snapshots and conservative old/new report/result allowance.
        // Preflight before collecting/cloning any new snapshot or report.
        let bytes = (values.len() as u64)
            .checked_mul(72)
            .and_then(|n| n.checked_add(self.snapshot.len() as u64 * 8))
            .and_then(|n| n.checked_add(262_144))
            .ok_or_else(|| error("memory_limit", "Automatic fitting storage estimate overflow."))?;
        if bytes > self.options.max_memory_bytes {
            return Err(error("memory_limit", "Automatic fitting snapshots, reports and scratch exceed memory budget."));
        }
        if self.snapshot.len() == values.len()
            && self.snapshot.iter().zip(values).all(|(bits, value)| *bits == value.to_bits())
            && let Some(previous) = &self.result
        {
            let mut cached = previous.clone();
            cached.cache_hit = true;
            return Ok(cached);
        }
        let families = [FitFamily::Normal, FitFamily::Uniform, FitFamily::Exponential, FitFamily::Lognormal, FitFamily::Laplace, FitFamily::Cauchy];
        let ranking = rank_fits(values, &families, RankingCriterion::Bic, &self.options, control)?;
        let selected = ranking
            .fits
            .first()
            .map(|(_, fit)| fit.clone())
            .ok_or_else(|| error("no_successful_fit", "All Solvercraft automatic-fit candidates rejected this snapshot."))?;
        cancelled(control)?;
        let result = AutomaticFitResult {
            selected,
            ranking,
            candidate_families: AUTOMATIC_FIT_FAMILIES.iter().map(|name| (*name).to_owned()).collect(),
            policy_version: AUTOMATIC_FIT_POLICY.into(),
            cache_hit: false,
            warnings: vec![
                "Solvercraft continuous-only six-family BIC policy; proprietary automatic-fit defaults are unverified.".into(),
                "Best means lowest BIC among successful candidates, not a universal best-distribution or global MLE certificate.".into(),
                "Call again with the current data snapshot after host-linked cells change; no external workbook watcher is installed.".into(),
            ],
        };
        let mut snapshot = Vec::new();
        reserve(&mut snapshot, values.len())?;
        snapshot.extend(values.iter().map(|value| value.to_bits()));
        self.snapshot = snapshot;
        self.result = Some(result.clone());
        Ok(result)
    }
}

/// Automatic fitting with the explicit Solvercraft default policy and limits.
/// Use [`PsiFitCache`] to reuse unchanged snapshots and recompute changed data.
pub fn psi_fit(values: &[f64], control: &dyn RunControl) -> Result<AutomaticFitResult> {
    PsiFitCache::default().fit(values, control)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrequencyKind {
    LiteralOrContinuous,
    DiscreteSample,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitPolicy {
    PerOccurrenceAfterDeductible,
    PerOccurrenceBeforeDeductible,
    AggregateAfterDeductible,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MakeInputLimits {
    pub max_events: u64,
    pub max_evaluations: u64,
    pub max_work: u64,
    pub max_memory_bytes: u64,
}
impl Default for MakeInputLimits {
    fn default() -> Self {
        Self { max_events: 10_000, max_evaluations: 1_000_000, max_work: 10_000_000, max_memory_bytes: 64 * 1024 * 1024 }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MakeInputRequest {
    /// None explicitly means no simulation context; no worksheet base-case is guessed.
    pub address: Option<VectorAddress>,
    pub frequency_kind: FrequencyKind,
    #[serde(default)]
    pub deductible: Option<f64>,
    #[serde(default)]
    pub limit: Option<f64>,
    /// Required when a limit is supplied: the published order is ambiguous.
    #[serde(default)]
    pub limit_policy: Option<LimitPolicy>,
    #[serde(default)]
    pub limits: MakeInputLimits,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventBinding {
    #[serde(with = "solvercraft_types::decimal_seed")]
    pub id: u64,
    pub distribution: Distribution,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventAddress {
    #[serde(with = "solvercraft_types::decimal_seed")]
    pub seed: u64,
    #[serde(with = "solvercraft_types::decimal_seed")]
    pub binding: u64,
    #[serde(with = "solvercraft_types::decimal_seed")]
    pub trial: u64,
    #[serde(with = "solvercraft_types::decimal_seed")]
    pub occurrence: u64,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventRole {
    Frequency,
    Occurrence,
}
struct WorkState {
    evaluations: u64,
    work: u64,
    limits: MakeInputLimits,
    stop: Option<SolverError>,
}
impl WorkState {
    fn charge(&mut self, work: u64, evaluations: u64, control: &dyn RunControl) -> Result<()> {
        if let Some(error) = &self.stop {
            return Err(error.clone());
        }
        let issue = if control.is_cancelled() {
            Some(error("cancelled", "Event expression cancelled."))
        } else if self.work.checked_add(work).is_none_or(|v| v > self.limits.max_work) {
            Some(error("work_limit", "Event expression work budget exhausted."))
        } else if self.evaluations.checked_add(evaluations).is_none_or(|v| v > self.limits.max_evaluations) {
            Some(error("evaluation_limit", "Event expression evaluation budget exhausted."))
        } else {
            None
        };
        if let Some(error) = issue {
            self.stop = Some(error.clone());
            return Err(error);
        }
        self.work += work;
        self.evaluations += evaluations;
        Ok(())
    }
}
/// One complete expression occurrence. Repeated reads of a registered binding
/// are stable within this context; another occurrence has a separate address.
pub struct EventContext<'a> {
    address: EventAddress,
    role: EventRole,
    bindings: &'a [(u64, math::PreparedDistribution)],
    cache: &'a mut Vec<(u64, f64)>,
    state: &'a mut WorkState,
    control: &'a dyn RunControl,
}
impl EventContext<'_> {
    pub fn address(&self) -> EventAddress {
        self.address
    }
    pub fn role(&self) -> EventRole {
        self.role
    }
    /// Host callbacks should charge expensive external expression operations;
    /// arbitrary user code cannot be preempted while it ignores this context.
    pub fn charge_work(&mut self, units: u64) -> Result<()> {
        self.state.charge(units, 0, self.control)
    }
    pub fn sample(&mut self, binding: u64) -> Result<f64> {
        self.state.charge(1, 0, self.control)?;
        if let Some((_, value)) = self.cache.iter().find(|(id, _)| *id == binding) {
            return Ok(*value);
        }
        let index = self
            .bindings
            .binary_search_by_key(&binding, |(id, _)| *id)
            .map_err(|_| error("unknown_binding", "Expression requested an unregistered event binding."))?;
        self.state.charge(1, 1, self.control)?;
        let domain = match self.role {
            EventRole::Frequency => 0x4d49_4652_4551,
            EventRole::Occurrence => 0x4d49_4556_454e,
        };
        let seed = math::random::addressed_u64(self.address.seed, self.address.binding, self.address.trial, domain);
        let value = self.bindings[index].1.sample(seed, binding, self.address.occurrence)?;
        self.cache.push((binding, value));
        Ok(value)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MakeInputResult {
    pub termination: Termination,
    pub frequency_value: Option<f64>,
    pub requested_events: Option<u64>,
    pub completed_events: u64,
    /// Only populated after all requested expression occurrences complete.
    pub value: Option<f64>,
    pub partial_sum: Option<f64>,
    pub evaluations: u64,
    pub work: u64,
    pub error: Option<SolverError>,
    pub stream_version: String,
    pub limit_policy: Option<LimitPolicy>,
    pub warnings: Vec<String>,
}
fn make_result(
    request: &MakeInputRequest,
    state: &WorkState,
    sum: &solvercraft_model::ExactProducts,
    frequency: Option<f64>,
    requested: Option<u64>,
    completed: u64,
    problem: Option<SolverError>,
) -> MakeInputResult {
    let problem = state.stop.clone().or(problem);
    let termination = match problem.as_ref().map(|e| e.code.as_str()) {
        None => Termination::Completed,
        Some("cancelled") => Termination::Cancelled,
        Some("work_limit" | "evaluation_limit") => Termination::EvaluationLimit,
        Some("event_limit") => Termination::IterationLimit,
        Some("memory_limit") => Termination::MemoryLimit,
        _ => Termination::NumericalFailure,
    };
    MakeInputResult{termination,frequency_value:frequency,requested_events:requested,completed_events:completed,value:None,partial_sum:sum.clone().finish().ok(),evaluations:state.evaluations,work:state.work,
        error:problem,stream_version:EVENT_STREAM_VERSION.into(),limit_policy:request.limit_policy,warnings:vec!["Explicit SDK event semantics; ambiguous limit ordering is chosen by the caller. No vendor random stream, worksheet base-case or proprietary catalog parity is claimed.".into()]}
}
/// Compound a complete arbitrary host expression, with one fresh event context
/// per occurrence. A supplied limit MUST have an explicit composition policy.
/// Literal/continuous frequencies floor; discrete samples below1 clamp to1.
/// Fractional discrete counts>=1 are refused because that behavior is unverified.
pub fn psi_make_input<Frequency, Event>(
    request: &MakeInputRequest,
    bindings: &[EventBinding],
    control: &dyn RunControl,
    frequency: Frequency,
    mut expression: Event,
) -> Result<MakeInputResult>
where
    Frequency: FnOnce(&mut EventContext<'_>) -> Result<f64>,
    Event: FnMut(&mut EventContext<'_>) -> Result<f64>,
{
    let address = request.address.ok_or_else(|| error("simulation_context_required", "MakeInput needs an explicit simulation address."))?;
    if request.limits.max_events > 100_000
        || bindings.len() > 1024
        || request.limits.max_evaluations > 10_000_000
        || request.limits.max_work > 1_000_000_000
        || request.deductible.is_some_and(|v| !v.is_finite())
        || request.limit.is_some_and(|v| !v.is_finite())
    {
        return Err(error("invalid_make_input", "Event limits and deduction/limit parameters must be finite and bounded."));
    }
    if request.limit.is_some() != request.limit_policy.is_some() {
        return Err(error("ambiguous_limit_policy", "Provide both a limit and an explicit ordering/scope policy, or neither."));
    }
    let mut state = WorkState { evaluations: 0, work: 0, limits: request.limits.clone(), stop: None };
    let mut sum = solvercraft_model::ExactProducts::default();
    let ready = if request.limits.max_work == 0 {
        state.charge(1, 0, control)
    } else if request.limits.max_evaluations == 0 {
        state.charge(0, 1, control)
    } else {
        state.charge(0, 0, control)
    };
    if let Err(problem) = ready {
        return Ok(make_result(request, &state, &sum, None, None, 0, Some(problem)));
    }
    let mut bytes = 8192_u64 + bindings.len() as u64 * 128;
    for binding in bindings {
        if let Err(problem) = state.charge(0, 0, control) {
            return Ok(make_result(request, &state, &sum, None, None, 0, Some(problem)));
        }
        add_size(&mut bytes, distribution_memory(&binding.distribution, request.limits.max_memory_bytes)?)?;
        if bytes > request.limits.max_memory_bytes {
            return Err(error("memory_limit", "Event preparation/cache exceeds memory budget before allocation."));
        }
    }
    if bytes > request.limits.max_memory_bytes {
        return Err(error("memory_limit", "Event preparation/cache exceeds memory budget before allocation."));
    }
    let mut ordered = Vec::new();
    reserve(&mut ordered, bindings.len())?;
    ordered.extend(bindings.iter());
    ordered.sort_unstable_by_key(|b| b.id);
    if ordered.windows(2).any(|w| w[0].id == w[1].id) {
        return Err(error("duplicate_binding", "Each registered event binding must have one immutable definition."));
    }
    let mut prepared = Vec::new();
    reserve(&mut prepared, ordered.len())?;
    for binding in ordered {
        if let Err(problem) = state.charge(0, 0, control) {
            return Ok(make_result(request, &state, &sum, None, None, 0, Some(problem)));
        }
        prepared.push((binding.id, math::PreparedDistribution::new(&binding.distribution)?));
    }
    let mut cache = Vec::new();
    reserve(&mut cache, bindings.len())?;
    if let Err(problem) = state.charge(1, 1, control) {
        return Ok(make_result(request, &state, &sum, None, None, 0, Some(problem)));
    }
    control.progress(&ProgressEvent {
        stage: "make_input_frequency".into(),
        iterations: 0,
        evaluations: state.evaluations,
        nodes: 0,
        best_objective: None,
    });
    let frequency_value = {
        let mut context = EventContext {
            address: EventAddress { seed: address.seed, binding: address.binding, trial: address.trial, occurrence: 0 },
            role: EventRole::Frequency,
            bindings: &prepared,
            cache: &mut cache,
            state: &mut state,
            control,
        };
        frequency(&mut context)
    };
    let raw = match frequency_value {
        Ok(value) if value.is_finite() => value,
        Ok(_) => {
            return Ok(make_result(
                request,
                &state,
                &sum,
                None,
                None,
                0,
                Some(error("frequency_domain", "Frequency expression must return a finite value.")),
            ));
        }
        Err(problem) => return Ok(make_result(request, &state, &sum, None, None, 0, Some(problem))),
    };
    if let Err(problem) = state.charge(0, 0, control) {
        return Ok(make_result(request, &state, &sum, Some(raw), None, 0, Some(problem)));
    }
    let count = match request.frequency_kind {
        FrequencyKind::LiteralOrContinuous => {
            if raw < 0.0 {
                return Ok(make_result(
                    request,
                    &state,
                    &sum,
                    Some(raw),
                    None,
                    0,
                    Some(error("frequency_domain", "Literal/continuous frequency cannot be negative.")),
                ));
            }
            raw.floor()
        }
        FrequencyKind::DiscreteSample => {
            if raw < 1.0 {
                1.0
            } else if raw.fract() != 0.0 {
                return Ok(make_result(
                    request,
                    &state,
                    &sum,
                    Some(raw),
                    None,
                    0,
                    Some(error("frequency_semantics", "Noninteger discrete frequency>=1 has no verified count convention.")),
                ));
            } else {
                raw
            }
        }
    };
    if count > request.limits.max_events as f64 {
        return Ok(make_result(
            request,
            &state,
            &sum,
            Some(raw),
            None,
            0,
            Some(error("event_limit", "Requested frequency exceeds the bounded event count.")),
        ));
    }
    let count = count as u64;
    let deductible = request.deductible.unwrap_or(0.0);
    let mut completed = 0;
    for occurrence in 0..count {
        if let Err(problem) = state.charge(1, 1, control) {
            return Ok(make_result(request, &state, &sum, Some(raw), Some(count), completed, Some(problem)));
        }
        cache.clear();
        let sampled = {
            let mut context = EventContext {
                address: EventAddress { seed: address.seed, binding: address.binding, trial: address.trial, occurrence },
                role: EventRole::Occurrence,
                bindings: &prepared,
                cache: &mut cache,
                state: &mut state,
                control,
            };
            expression(&mut context)
        };
        let value = match sampled {
            Ok(value) if value.is_finite() => value,
            Ok(_) => {
                return Ok(make_result(
                    request,
                    &state,
                    &sum,
                    Some(raw),
                    Some(count),
                    completed,
                    Some(error("event_domain", "An event expression returned a nonfinite value.")),
                ));
            }
            Err(problem) => return Ok(make_result(request, &state, &sum, Some(raw), Some(count), completed, Some(problem))),
        };
        if let Err(problem) = state.charge(0, 0, control) {
            return Ok(make_result(request, &state, &sum, Some(raw), Some(count), completed, Some(problem)));
        }
        let add = match (request.limit, request.limit_policy) {
            (Some(limit), Some(LimitPolicy::PerOccurrenceAfterDeductible)) => {
                let mut compare = solvercraft_model::ExactProducts::default();
                compare.add(value)?;
                compare.add(-deductible)?;
                compare.add(-limit)?;
                if compare.exact_sign()? == std::cmp::Ordering::Greater { sum.add(limit) } else { sum.add(value).and_then(|()| sum.add(-deductible)) }
            }
            (Some(limit), Some(LimitPolicy::PerOccurrenceBeforeDeductible)) => sum.add(value.min(limit)).and_then(|()| sum.add(-deductible)),
            _ => sum.add(value).and_then(|()| sum.add(-deductible)),
        };
        if let Err(problem) = add {
            return Ok(make_result(request, &state, &sum, Some(raw), Some(count), completed, Some(problem)));
        }
        completed += 1;
        control.progress(&ProgressEvent {
            stage: "make_input_events".into(),
            iterations: completed,
            evaluations: state.evaluations,
            nodes: 0,
            best_objective: None,
        });
    }
    let mut result = make_result(request, &state, &sum, Some(raw), Some(count), completed, None);
    let value = if let (Some(limit), Some(LimitPolicy::AggregateAfterDeductible)) = (request.limit, request.limit_policy) {
        let mut compare = sum.clone();
        compare.add(-limit)?;
        if compare.exact_sign()? == std::cmp::Ordering::Greater { Ok(limit) } else { sum.finish() }
    } else {
        sum.finish()
    };
    match value {
        Ok(value) => result.value = Some(value),
        Err(problem) => {
            result.termination = Termination::NumericalFailure;
            result.error = Some(problem);
        }
    }
    Ok(result)
}
