#[allow(dead_code)]
#[allow(unused_variables)]
#[allow(clippy::needless_range_loop)]
#[cfg(test)]
mod tests {
  use rayon::prelude::*;

  #[cfg(debug_assertions)]
  pub fn is_debug() -> bool {
    true
  }

  #[cfg(not(debug_assertions))]
  pub fn is_debug() -> bool {
    return false;
  }

  fn levenshtein_v0(a: &str, b: &str) -> usize {
    let mut costs = vec![0; b.len() + 1];

    for i in 0..=a.len() {
      let mut last_value = i;
      for j in 0..=b.len() {
        if i == 0 {
          costs[j] = j;
        } else if j > 0 {
          let new_value = costs[j - 1];
          if a.as_bytes()[i - 1] != b.as_bytes()[j - 1] {
            costs[j - 1] =
              std::cmp::min(std::cmp::min(costs[j - 1], last_value), costs[j])
                + 1;
          } else {
            costs[j - 1] = last_value;
          }
          last_value = new_value;
        }
      }
      if i > 0 {
        costs[b.len()] = last_value;
      }
    }

    costs[b.len()]
  }

  fn levenshtein_v1(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();

    // Initialize the cost matrix
    let mut costs: Vec<Vec<usize>> = vec![vec![0; b_len + 1]; a_len + 1];

    // Fill the first row and first column
    for i in 0..=a_len {
      costs[i][0] = i;
    }
    for j in 0..=b_len {
      costs[0][j] = j;
    }

    // Clone costs for use in the closure
    let costs_clone = costs.clone();

    // Parallel computation of the cost matrix
    costs.par_iter_mut().enumerate().skip(1).for_each(|(i, row)| {
      for j in 1..=b_len {
        let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
        row[j] = std::cmp::min(
          std::cmp::min(costs_clone[i - 1][j] + 1, row[j - 1] + 1),
          costs_clone[i - 1][j - 1] + cost,
        );
      }
    });

    costs[a_len][b_len]
  }

  fn levenshtein_v2(a: &str, b: &str) -> usize {
    let len_a = a.len();
    let len_b = b.len();
    let mut matrix = vec![vec![0; len_b + 1]; len_a + 1];

    for i in 0..=len_a {
      matrix[i][0] = i;
    }

    for j in 0..=len_b {
      matrix[0][j] = j;
    }

    for j in 1..=len_b {
      for i in 1..=len_a {
        if a.chars().nth(i - 1) == b.chars().nth(j - 1) {
          matrix[i][j] = matrix[i - 1][j - 1];
        } else {
          matrix[i][j] = 1
            + std::cmp::min(
              matrix[i - 1][j - 1],
              std::cmp::min(matrix[i][j - 1], matrix[i - 1][j]),
            );
        }
      }
    }

    matrix[len_a][len_b]
  }

  fn similarity_percentage(a: &str, b: &str) -> f64 {
    let max_len = std::cmp::max(a.len(), b.len());
    if max_len == 0 {
      return 100.0;
    }

    let distance = levenshtein_v0(a, b);
    let similarity = 1.0 - (distance as f64 / max_len as f64);

    similarity * 100.0
  }

  #[test]
  fn test_end_to_end() {
    // Path to the reference input PDF file. Relative to this crate's directory
    // (cargo runs a test with the package root as its working directory), which
    // is packages/cli-pdf-to-text — so test-data/, which sits at the repository
    // root beside packages/, is two levels up.
    let input_path = "../../test-data/pdf/pdfreference1.7old.pdf";

    // Path to the reference output text file.
    let reference_output_path =
      "../../test-data/txt/pdftotext-24.02.0-pdfreference1.7old.pdf.txt";

    // println!("{:?}", std::env::current_dir());
    // println!("{:?}", std::env::current_exe());
    // println!("{:?}", is_debug());

    // for (key, value) in std::env::vars() {
    //   println!("{key}: {value}");
    // }

    // Read the reference output text file.
    let reference_output =
      std::fs::read_to_string(reference_output_path).unwrap();

    // target/ lives at the workspace root, which is two levels above this
    // crate (packages/cli-pdf-to-text), not one.
    let binding = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let binary_path_base = binding.parent().unwrap().parent().unwrap();

    let binary_path = if is_debug() {
      binary_path_base.join("target/debug/cli-pdf-to-text")
    } else {
      binary_path_base.join("target/release/cli-pdf-to-text")
    };

    // Run the command on the reference input PDF file.
    let output = std::process::Command::new(binary_path)
      // .arg(input_path)
      .output()
      .expect("Failed to execute command");

    // Convert the output of your program to a String.
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    let success = output.status.success();

    println!("{stdout}");
    println!("{stderr}");
    println!("{success}");

    if (!success) {
      // panic!("command failed");
    }

    // TODO cant test this here, move it to cli-pdf-to-text or cli-justify
    // TODO write a smaller reference pdf file with accompanying reference text
    // file in latex
    println!("{}", reference_output.len());
    println!("{}", stdout.len());

    // TODO this crashes signal:0, SIGKILL: kill, or just does not finish
    // let similarity = similarity_percentage(&reference_output, &stdout);
    // println!("Similarity: {:.2}%", similarity);

    // Compare the output of your program to the reference output text file.
    // assert_eq!(output_str, reference_output);

    // panic!();
  }
}
