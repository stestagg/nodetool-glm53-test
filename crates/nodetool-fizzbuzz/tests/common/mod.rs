//! Oracles and fixtures shared by the test binaries.

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

/// The shipped graph with the counter's stop raised, as a temp file —
/// the hand-written long variant the tests invite.
#[allow(dead_code)]
pub fn long_counter(stop: &str) -> std::path::PathBuf {
    let shipped = concat!(env!("CARGO_MANIFEST_DIR"), "/graphs/fizzbuzz.yml");
    let hand_written = std::fs::read_to_string(shipped)
        .expect("the shipped graph is readable")
        .replace("stop: 100", &format!("stop: {stop}"));
    assert!(
        hand_written.contains(&format!("stop: {stop}")),
        "the shipped graph carries the counter's stop"
    );
    let path = std::env::temp_dir().join(format!("nodetool-fizzbuzz-{stop}.yml"));
    std::fs::write(&path, hand_written).expect("the variant is written");
    path
}
