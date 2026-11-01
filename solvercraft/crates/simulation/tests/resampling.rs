#![allow(clippy::unwrap_used)]
use solvercraft_simulation::resampling::*;
use solvercraft_types::{NoControl, RunControl};
fn address(trial: u64) -> VectorAddress {
    VectorAddress { seed: 17, binding: 3, trial }
}
#[test]
fn resampling_preserves_intact_source_rows_or_columns_and_replays_exact_trials() {
    let limits = ResamplingLimits::default();
    let rows = vec![vec![1.0, 2.0, 3.0], vec![11.0, 12.0, 13.0]];
    let data = PreparedTable::new(&rows, &limits, &NoControl).unwrap();
    let values = PreparedValues::new(&[5.0, 7.0, 9.0], &limits, &NoControl).unwrap();
    for trial in 0..64 {
        let row = data.mv_resample(VectorOrientation::Row, address(trial), &NoControl).unwrap();
        assert!(rows.contains(&row));
        assert_eq!(row, data.mv_resample(VectorOrientation::Row, address(trial), &NoControl).unwrap());
        let column = data.mv_resample(VectorOrientation::Column, address(trial), &NoControl).unwrap();
        assert!([vec![1.0, 11.0], vec![2.0, 12.0], vec![3.0, 13.0]].contains(&column));
        assert!([5.0, 7.0, 9.0].contains(&values.resample(address(trial), &NoControl).unwrap()));
    }
    assert_eq!(values.sip(1, &NoControl).unwrap(), 7.0);
    assert!(values.sip(3, &NoControl).is_err());
    assert_eq!(data.slurp(1, 3, &NoControl).unwrap(), 13.0);
    assert!(data.slurp(0, 0, &NoControl).is_err());
    assert!(data.slurp(2, 1, &NoControl).is_err());
    assert!(data.slurp(0, 4, &NoControl).is_err());
}
#[test]
fn shuffle_is_a_full_multiset_permutation_without_replacement() {
    let limits = ResamplingLimits::default();
    let input = vec![vec![0.0, -0.0], vec![1.0, 1.0]];
    let data = PreparedTable::new(&input, &limits, &NoControl).unwrap();
    let mut expected = input.iter().flatten().map(|x| x.to_bits()).collect::<Vec<_>>();
    expected.sort_unstable();
    let mut distinct = std::collections::BTreeSet::new();
    for trial in 0..32 {
        let result = data.mv_shuffle(address(trial), &NoControl).unwrap();
        assert_eq!(result, data.mv_shuffle(address(trial), &NoControl).unwrap());
        let mut bits = result.iter().flatten().map(|x| x.to_bits()).collect::<Vec<_>>();
        distinct.insert(bits.clone());
        bits.sort_unstable();
        assert_eq!(bits, expected);
    }
    assert!(distinct.len() > 1);
}
struct Cancel;
impl RunControl for Cancel {
    fn is_cancelled(&self) -> bool {
        true
    }
}
#[test]
fn resampling_rejects_ragged_nonfinite_cancelled_or_unbudgeted_work() {
    let limits = ResamplingLimits::default();
    assert!(PreparedValues::new(&[], &limits, &NoControl).is_err());
    assert!(PreparedValues::new(&[f64::NAN], &limits, &NoControl).is_err());
    assert!(PreparedTable::new(&[vec![1.0], vec![1.0, 2.0]], &limits, &NoControl).is_err());
    assert!(PreparedTable::new(&[vec![1.0]], &limits, &Cancel).is_err());
    let tight = ResamplingLimits { max_memory_bytes: 300, ..limits };
    let data = PreparedTable::new(&[vec![1.0]], &tight, &NoControl).unwrap();
    assert!(data.mv_shuffle(address(0), &NoControl).is_err());
    assert!(data.mv_resample(VectorOrientation::Row, address(0), &NoControl).is_err());
    assert_eq!(data.slurp(0, 1, &NoControl).unwrap(), 1.0);
}
