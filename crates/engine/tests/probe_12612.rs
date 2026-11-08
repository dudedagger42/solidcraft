//! Probe for #12612 (not for a PR): replays the script at PROBE12612_SCRIPT.
use serde_json::Value;
use solidcraft_engine::Session;

#[test]
#[ignore]
fn probe_12612() {
    let path = std::env::var("PROBE12612_SCRIPT").expect("PROBE12612_SCRIPT");
    let script: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut s = Session::default();
    for c in script.as_array().into_iter().flatten() {
        let (id, p) = (c["command"].as_str().unwrap_or_default(), &c["params"]);
        eprintln!("STEP {id}: {:?}", s.execute(id, p).map(|_| ()));
    }
}
