//! Oracles shared by the test binaries.

/// The fizzbuzz string for a count — the sequence's definition, restated
/// apart from the implementation under test.
pub fn fizzbuzz_line(count: i32) -> String {
    match (count % 3 == 0, count % 5 == 0) {
        (true, true) => "FizzBuzz".to_owned(),
        (true, false) => "Fizz".to_owned(),
        (false, true) => "Buzz".to_owned(),
        (false, false) => count.to_string(),
    }
}
