//! A panic's message never reaches the report: it can quote what caused it.
//! Its own test binary, since a panic hook is process-wide.

use std::sync::{Arc, Mutex};

use campbooks_server::telemetry::withheld_panic_report;

#[test]
fn panic_messages_are_withheld() {
    let reports = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&reports);
    std::panic::set_hook(Box::new(move |info| {
        if let Ok(mut reports) = sink.lock() {
            reports.push(withheld_panic_report(info));
        }
    }));
    let query = "older_than:5é boiler repair for ana@example.com";
    let caught = std::panic::catch_unwind(|| {
        // The byte-slicing panic the search parser once had quoted its input.
        let _ = &query[..13];
    });
    let _ = std::panic::take_hook();
    assert!(caught.is_err());
    let reports = reports.lock().unwrap();
    assert_eq!(reports.len(), 1);
    assert!(reports[0].contains("panic_hook.rs"), "{}", reports[0]);
    for leaked in ["boiler", "ana@example.com", "5é"] {
        assert!(!reports[0].contains(leaked), "{}", reports[0]);
    }
}
