// Tests for the Kokoro engine, grouped by what they exercise:
//   - `synthesis`: real synthesis against the downloaded model (#[ignore]);
//   - `alignment`: pure timing/apportionment math (CI);
//   - `assemble`: punctuation/pause-token regression with an injected
//     phonemizer (CI);
//   - `espeak`: the same guard run end to end against the real espeak (CI,
//     skips when espeak-ng-data is unavailable).

mod alignment;
mod assemble;
mod espeak;
mod synthesis;
