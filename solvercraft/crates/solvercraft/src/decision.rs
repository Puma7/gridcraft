//! Finite decision trees, expected utility and explicitly timed information.
use serde::{Deserialize, Serialize};
use solvercraft_model::CompensatedSum;
use solvercraft_types::{Result, SolverError};
use std::collections::VecDeque;
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Utility {
    Linear {},
    Exponential { risk_tolerance: f64 },
}
impl Utility {
    fn validate(self) -> Result<()> {
        if let Self::Exponential { risk_tolerance } = self
            && (!risk_tolerance.is_finite() || risk_tolerance <= 0.0)
        {
            return Err(invalid("Risk tolerance must be positive and finite"));
        }
        Ok(())
    }
    fn value(self, payoff: f64) -> Result<f64> {
        let value = match self {
            Self::Linear {} => payoff,
            Self::Exponential { risk_tolerance } => -(-payoff / risk_tolerance).exp(),
        };
        if !value.is_finite() || (matches!(self, Self::Exponential { .. }) && value == 0.0) {
            return Err(invalid("Utility exceeds supported finite precision"));
        }
        Ok(value)
    }
    fn cost(self, utility: f64, cost: f64) -> Result<f64> {
        let value = match self {
            Self::Linear {} => utility - cost,
            Self::Exponential { risk_tolerance } => utility * (cost / risk_tolerance).exp(),
        };
        if !value.is_finite() || (matches!(self, Self::Exponential { .. }) && value == 0.0) {
            return Err(invalid("Edge cost exceeds supported finite utility precision"));
        }
        Ok(value)
    }
    fn equivalent(self, utility: f64) -> Result<f64> {
        let value = match self {
            Self::Linear {} => utility,
            Self::Exponential { risk_tolerance } => -risk_tolerance * (-utility).ln(),
        };
        if value.is_finite() { Ok(value) } else { Err(invalid("Certainty equivalent is not finite")) }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreeEdge {
    pub child: usize,
    pub cost: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChanceEdge {
    pub child: usize,
    pub cost: f64,
    pub probability: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DecisionNode {
    Terminal { payoff: f64 },
    Chance { outcomes: Vec<ChanceEdge> },
    Decision { alternatives: Vec<TreeEdge> },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionTree {
    pub root: usize,
    pub nodes: Vec<DecisionNode>,
    pub utility: Utility,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeEvaluation {
    pub expected_utility: f64,
    pub certainty_equivalent: f64,
    pub node_utilities: Vec<f64>,
    pub choices: Vec<Option<usize>>,
}
fn invalid(message: &str) -> SolverError {
    SolverError::new("invalid_decision_tree", message)
}
fn reserve<T>(v: &mut Vec<T>, n: usize) -> Result<()> {
    v.try_reserve_exact(n).map_err(|_| SolverError::new("allocation", "Cannot allocate decision tree"))
}
fn edges(node: &DecisionNode) -> usize {
    match node {
        DecisionNode::Terminal { .. } => 0,
        DecisionNode::Chance { outcomes } => outcomes.len(),
        DecisionNode::Decision { alternatives } => alternatives.len(),
    }
}
fn children(node: &DecisionNode) -> Box<dyn Iterator<Item = (usize, f64)> + '_> {
    match node {
        DecisionNode::Terminal { .. } => Box::new(std::iter::empty()),
        DecisionNode::Chance { outcomes } => Box::new(outcomes.iter().map(|e| (e.child, e.cost))),
        DecisionNode::Decision { alternatives } => Box::new(alternatives.iter().map(|e| (e.child, e.cost))),
    }
}
/// Backwards dynamic programming on a bounded acyclic tree/DAG. A decision node
/// observes precisely the path reaching it; sharing a node shares its policy.
pub fn evaluate(tree: &DecisionTree, max_memory_bytes: u64) -> Result<TreeEvaluation> {
    tree.utility.validate()?;
    if tree.nodes.is_empty() || tree.nodes.len() > 100_000 || tree.root >= tree.nodes.len() {
        return Err(invalid("Invalid tree size or root"));
    }
    let mut edge_count = 0usize;
    for node in &tree.nodes {
        edge_count = edge_count.checked_add(edges(node)).ok_or_else(|| invalid("Edge overflow"))?;
    }
    if edge_count > 1_000_000 {
        return Err(invalid("Too many tree edges"));
    }
    let bytes = tree.nodes.len().checked_mul(192).and_then(|n| n.checked_add(edge_count.checked_mul(32)?));
    if bytes.is_none_or(|n| n as u64 > max_memory_bytes) {
        return Err(SolverError::new("memory_limit", "Decision tree exceeds memory budget before allocation"));
    }
    for node in &tree.nodes {
        if !matches!(node, DecisionNode::Terminal { .. }) && edges(node) == 0 {
            return Err(invalid("Nonterminal node has no outgoing edge"));
        }
        for (child, cost) in children(node) {
            if child >= tree.nodes.len() || !cost.is_finite() {
                return Err(invalid("Invalid edge handle or cost"));
            }
        }
        match node {
            DecisionNode::Terminal { payoff } => {
                if !payoff.is_finite() {
                    return Err(invalid("Terminal payoff must be finite"));
                }
            }
            DecisionNode::Chance { outcomes } => {
                let mut total = CompensatedSum::default();
                for edge in outcomes {
                    if !edge.probability.is_finite() || edge.probability < 0.0 {
                        return Err(invalid("Probabilities must be finite and nonnegative"));
                    }
                    total.add(edge.probability)?;
                }
                if (total.finish()? - 1.0).abs() > 1e-12 {
                    return Err(invalid("Chance probabilities must sum to one"));
                }
            }
            DecisionNode::Decision { .. } => {}
        }
    }
    let mut parents = Vec::new();
    reserve(&mut parents, tree.nodes.len())?;
    parents.resize_with(tree.nodes.len(), Vec::new);
    let mut remaining = Vec::new();
    reserve(&mut remaining, tree.nodes.len())?;
    let mut ready = VecDeque::new();
    ready.try_reserve(tree.nodes.len()).map_err(|_| invalid("Cannot allocate node queue"))?;
    for (i, node) in tree.nodes.iter().enumerate() {
        remaining.push(edges(node));
        if edges(node) == 0 {
            ready.push_back(i);
        }
        for (child, _) in children(node) {
            parents[child].push(i);
        }
    }
    let mut utilities = Vec::new();
    reserve(&mut utilities, tree.nodes.len())?;
    utilities.resize(tree.nodes.len(), 0.0);
    let mut choices = Vec::new();
    reserve(&mut choices, tree.nodes.len())?;
    choices.resize(tree.nodes.len(), None);
    let mut completed = 0;
    while let Some(index) = ready.pop_front() {
        utilities[index] = match &tree.nodes[index] {
            DecisionNode::Terminal { payoff } => tree.utility.value(*payoff)?,
            DecisionNode::Chance { outcomes } => {
                let mut values = Vec::new();
                reserve(&mut values, outcomes.len())?;
                let mut weights = Vec::new();
                reserve(&mut weights, outcomes.len())?;
                for edge in outcomes {
                    values.push(tree.utility.cost(utilities[edge.child], edge.cost)?);
                    weights.push(edge.probability);
                }
                crate::risk::weighted_mean(&values, &weights)?
            }
            DecisionNode::Decision { alternatives } => {
                let mut best = f64::NEG_INFINITY;
                for (choice, edge) in alternatives.iter().enumerate() {
                    let value = tree.utility.cost(utilities[edge.child], edge.cost)?;
                    if value > best {
                        best = value;
                        choices[index] = Some(choice);
                    }
                }
                best
            }
        };
        completed += 1;
        for parent in &parents[index] {
            remaining[*parent] -= 1;
            if remaining[*parent] == 0 {
                ready.push_back(*parent);
            }
        }
    }
    if completed != tree.nodes.len() {
        return Err(invalid("Decision graph contains a cycle"));
    }
    let expected_utility = utilities[tree.root];
    Ok(TreeEvaluation { expected_utility, certainty_equivalent: tree.utility.equivalent(expected_utility)?, node_utilities: utilities, choices })
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InformationProblem {
    pub state_probabilities: Vec<f64>,
    /// action_payoffs[action][state]. All actions are available before observing information.
    pub action_payoffs: Vec<Vec<f64>>,
    /// signal_likelihoods[signal][state], each state column sums to one.
    pub signal_likelihoods: Vec<Vec<f64>>,
    /// Paid before signal observation; deducted before applying utility.
    pub acquisition_cost: f64,
    pub utility: Utility,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InformationValue {
    pub baseline_action: usize,
    pub baseline_certainty_equivalent: f64,
    pub perfect_information_certainty_equivalent: f64,
    pub signal_actions: Vec<usize>,
    pub signal_certainty_equivalent: f64,
    pub net_value_of_signal: f64,
    pub value_of_perfect_information: f64,
}
/// Compare a pre-information action with policies chosen after a signal, and
/// perfect information. Acquisition cost is included exactly once, before utility.
pub fn information_value(problem: &InformationProblem, max_memory_bytes: u64) -> Result<InformationValue> {
    problem.utility.validate()?;
    let n = problem.state_probabilities.len();
    let a = problem.action_payoffs.len();
    let s = problem.signal_likelihoods.len();
    if n == 0 || a == 0 || s == 0 || n > 10_000 || a > 10_000 || s > 10_000 || !problem.acquisition_cost.is_finite() || problem.acquisition_cost < 0.0
    {
        return Err(invalid("Invalid information problem dimensions or cost"));
    }
    let cells = n.checked_mul(a.checked_add(s).ok_or_else(|| invalid("Dimension overflow"))?).ok_or_else(|| invalid("Dimension overflow"))?;
    if cells > 1_000_000
        || n.checked_mul(a).and_then(|x| x.checked_mul(s)).is_none_or(|x| x > 10_000_000)
        || cells.checked_mul(64).is_none_or(|x| x as u64 > max_memory_bytes)
    {
        return Err(SolverError::new("memory_limit", "Information problem exceeds work budget"));
    }
    if problem.action_payoffs.iter().any(|v| v.len() != n || v.iter().any(|x| !x.is_finite()))
        || problem.signal_likelihoods.iter().any(|v| v.len() != n || v.iter().any(|x| !x.is_finite() || *x < 0.0))
        || problem.state_probabilities.iter().any(|p| !p.is_finite() || *p < 0.0)
    {
        return Err(invalid("Invalid payoff, prior or likelihood"));
    }
    let mut priors = CompensatedSum::default();
    for p in &problem.state_probabilities {
        priors.add(*p)?;
    }
    if (priors.finish()? - 1.0).abs() > 1e-12 {
        return Err(invalid("Priors must sum to one"));
    }
    for state in 0..n {
        let mut total = CompensatedSum::default();
        for likelihood in &problem.signal_likelihoods {
            total.add(likelihood[state])?;
        }
        if (total.finish()? - 1.0).abs() > 1e-12 {
            return Err(invalid("Signal likelihood columns must sum to one"));
        }
    }
    // The validation tolerance accepts nearby probability simplexes. Normalize
    // both levels once so conditioning and marginalization use the same law.
    let priors = crate::risk::probability_weights(problem.state_probabilities.iter().copied())?;
    let mut likelihoods = Vec::new();
    reserve(&mut likelihoods, s)?;
    for _ in 0..s {
        let mut row = Vec::new();
        reserve(&mut row, n)?;
        row.resize(n, 0.0);
        likelihoods.push(row);
    }
    for state in 0..n {
        let column = crate::risk::probability_weights(problem.signal_likelihoods.iter().map(|row| row[state]))?;
        for (row, probability) in likelihoods.iter_mut().zip(column) {
            row[state] = probability;
        }
    }
    let mut baseline = f64::NEG_INFINITY;
    let mut baseline_action = 0;
    let mut perfect = Vec::new();
    reserve(&mut perfect, n)?;
    perfect.resize(n, f64::NEG_INFINITY);
    for (action, payoffs) in problem.action_payoffs.iter().enumerate() {
        let mut utilities = Vec::new();
        reserve(&mut utilities, n)?;
        for (state, payoff) in payoffs.iter().enumerate() {
            let u = problem.utility.value(*payoff)?;
            utilities.push(u);
            perfect[state] = perfect[state].max(u);
        }
        let value = crate::risk::weighted_mean(&utilities, &priors)?;
        if value > baseline {
            baseline = value;
            baseline_action = action;
        }
    }
    let perfect = crate::risk::weighted_mean(&perfect, &priors)?;
    let mut signal_actions = Vec::new();
    reserve(&mut signal_actions, s)?;
    let mut signal_utilities = Vec::new();
    let mut signal_masses = Vec::new();
    reserve(&mut signal_utilities, s)?;
    reserve(&mut signal_masses, s)?;
    for likelihood in &likelihoods {
        let joint: Vec<_> = likelihood.iter().zip(&priors).map(|(l, p)| l * p).collect();
        if likelihood.iter().zip(&priors).zip(&joint).any(|((l, p), v)| *l > 0.0 && *p > 0.0 && (*v == 0.0 || v.abs() < f64::MIN_POSITIVE)) {
            return Err(invalid("Signal mass exceeds supported precision"));
        }
        let mut mass = CompensatedSum::default();
        for p in &joint {
            mass.add(*p)?;
        }
        let mass = mass.finish()?;
        if mass == 0.0 {
            signal_actions.push(0);
            continue;
        }
        let mut best = f64::NEG_INFINITY;
        let mut choice = 0;
        for (action, payoffs) in problem.action_payoffs.iter().enumerate() {
            let utilities: Vec<_> = payoffs.iter().map(|p| problem.utility.value(*p - problem.acquisition_cost)).collect::<Result<_>>()?;
            let value = crate::risk::weighted_mean(&utilities, &joint)?;
            if value > best {
                best = value;
                choice = action;
            }
        }
        signal_actions.push(choice);
        signal_masses.push(mass);
        signal_utilities.push(best);
    }
    let baseline_ce = problem.utility.equivalent(baseline)?;
    let perfect_ce = problem.utility.equivalent(perfect)?;
    let signal_ce = problem.utility.equivalent(crate::risk::weighted_mean(&signal_utilities, &signal_masses)?)?;
    let net_value_of_signal = signal_ce - baseline_ce;
    let value_of_perfect_information = perfect_ce - baseline_ce;
    if !net_value_of_signal.is_finite() || !value_of_perfect_information.is_finite() {
        return Err(SolverError::new("numerical_failure", "Information value exceeds finite precision"));
    }
    Ok(InformationValue {
        baseline_action,
        baseline_certainty_equivalent: baseline_ce,
        perfect_information_certainty_equivalent: perfect_ce,
        signal_actions,
        signal_certainty_equivalent: signal_ce,
        net_value_of_signal,
        value_of_perfect_information,
    })
}
