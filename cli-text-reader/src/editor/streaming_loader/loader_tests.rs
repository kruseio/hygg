use super::loader::{load_order, loader_order};

#[test]
fn pattern_emits_forward_then_backward_blocks() {
  let order = load_order(50, 200);
  // First block: forward 10 starting at 51
  assert_eq!(&order[..10], &[51, 52, 53, 54, 55, 56, 57, 58, 59, 60]);
  // Second block: backward 10 from 49
  assert_eq!(&order[10..20], &[49, 48, 47, 46, 45, 44, 43, 42, 41, 40]);
  // Third block: forward 10 from 61
  assert_eq!(&order[20..30], &[61, 62, 63, 64, 65, 66, 67, 68, 69, 70]);
  // Fourth block: backward 10 from 39
  assert_eq!(&order[30..40], &[39, 38, 37, 36, 35, 34, 33, 32, 31, 30]);
}

#[test]
fn pattern_handles_edge_when_start_is_at_top() {
  let order = load_order(1, 25);
  // Backward exhausts immediately, so the sequence is pure forward.
  assert_eq!(order, (2..=25).collect::<Vec<_>>());
}

#[test]
fn pattern_handles_edge_when_start_is_near_bottom() {
  let order = load_order(95, 100);
  // First block forward: 96..=100 (only 5 available)
  assert_eq!(&order[..5], &[96, 97, 98, 99, 100]);
  // Then backward block of 10 from 94
  assert_eq!(&order[5..15], &[94, 93, 92, 91, 90, 89, 88, 87, 86, 85]);
}

#[test]
fn pattern_visits_every_page_exactly_once() {
  for total in [1, 2, 5, 11, 21, 50, 199] {
    for start in [1, total / 2, total] {
      if start == 0 || start > total {
        continue;
      }
      let order = load_order(start, total);
      let mut seen: Vec<usize> = order.clone();
      seen.sort_unstable();
      let mut expected: Vec<usize> = (1..=total).collect();
      expected.retain(|p| *p != start);
      assert_eq!(seen, expected, "total={total} start={start}");
    }
  }
}

#[test]
fn loader_renders_start_first_when_not_preloaded() {
  let skip = std::collections::HashSet::new();
  let order = loader_order(34, 501, &skip);

  assert_eq!(order.first(), Some(&34));
}

#[test]
fn loader_skips_preloaded_start_page() {
  let skip = std::collections::HashSet::from([34]);
  let order = loader_order(34, 501, &skip);

  assert_eq!(order.first(), Some(&35));
  assert!(!order.contains(&34));
}
