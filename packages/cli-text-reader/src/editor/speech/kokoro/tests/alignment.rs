// Pure timing/apportionment tests (no espeak, run in CI): per-token durations
// scaled to audio length, and the largest-remainder split that tiles a run's
// tokens across its words.

use super::super::align::{apportion, build_alignments};
use super::super::common::SAMPLE_RATE;

#[test]
fn build_alignments_scales_to_fast_audio_duration() {
  let word_map = vec![
    ("specific".to_string(), 0, 1),
    ("versions".to_string(), 1, 2),
    ("later".to_string(), 2, 3),
  ];
  let durations = vec![0.0, 40.0, 40.0, 40.0, 0.0];

  let alignments =
    build_alignments(&word_map, &durations, 1, 2.0, SAMPLE_RATE as usize);

  assert_eq!(alignments.len(), 3);
  assert!((alignments[0].start_sec - 0.0).abs() < 0.001);
  assert!((alignments[1].start_sec - 0.333).abs() < 0.01);
  assert!((alignments[2].start_sec - 0.667).abs() < 0.01);
  assert!((alignments[2].end_sec - 1.0).abs() < 0.001);
}

#[test]
fn apportion_always_sums_to_total() {
  for (raw, total) in [
    (vec![2usize, 7, 1], 10usize),
    (vec![1, 1, 1], 10),
    (vec![3, 1], 8),
    (vec![5], 5),
  ] {
    let got = apportion(&raw, total);
    assert_eq!(got.len(), raw.len());
    assert_eq!(got.iter().sum::<usize>(), total, "raw={raw:?} total={total}");
  }
}

#[test]
fn apportion_keeps_exact_counts_when_already_summing() {
  assert_eq!(apportion(&[2, 7, 1], 10), vec![2, 7, 1]);
}

#[test]
fn apportion_all_empty_dumps_to_last() {
  // A run that phonemized to nothing still has every token covered.
  assert_eq!(apportion(&[0, 0, 0], 4), vec![0, 0, 4]);
}
