//! PROBE (team2 fable, not for merge): the sliced rounded block of #13910 as `.smb` hex.
use serde_json::json;
use solidcraft_engine::Session;

#[test]
fn t2c_dump_sliced_block() {
    let mut s = Session::default();
    let mut run = |id: &str, p: serde_json::Value| s.execute(id, &p).unwrap();
    run("solid.box", json!({"length": 100.0, "width": 40.0, "height": 14.0, "body_name": "A"}));
    run("solid.fillet", json!({"edges": [[100.0, 0.0, 7.0], [100.0, 40.0, 7.0]], "radius": 10.0}));
    run("solid.fillet", json!({"edges": [[0.0, 40.0, 7.0], [0.0, 0.0, 7.0]], "radius": 15.0}));
    run("solid.fillet", json!({"edges": [[97.07106781186548, 2.928932188134526, 14.0], [97.07106781186548, 2.9289321881345267, 0.0]], "radius": 5.0}));
    run("solid.box", json!({"corner": [-5.0, -5.0, 12.8], "length": 110.0, "width": 50.0, "height": 5.0, "body_name": "B"}));
    run("solid.combine", json!({"target": "A", "tools": ["B"], "operation": "cut"}));
    let st = s.model.state();
    let body = &st.bodies[0].body;
    let path = std::env::temp_dir().join("t2c_13910_sliced_block.smb");
    body.smlib().unwrap().write(&path, false).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    println!("T2CSMB len {}", bytes.len());
    for chunk in bytes.chunks(60) {
        let hex: String = chunk.iter().map(|b| format!("{b:02x}")).collect();
        println!("T2CSMB:{hex}");
    }
}
