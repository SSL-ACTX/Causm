pub mod causm_acausal;
pub mod causm_acausal_debugging;
pub mod causm_advanced_loops;
pub mod causm_cli_toggles;
pub mod causm_collection;
pub mod causm_egc;
pub mod causm_encoding;
pub mod causm_entropic;
pub mod causm_expansion;
pub mod causm_http;
pub mod causm_import;
pub mod causm_isochronous;
pub mod causm_json;
pub mod causm_lease;
pub mod causm_match;
pub mod causm_oop;
pub mod causm_plugins_test;
pub mod causm_process;
pub mod causm_reconciliation;
pub mod causm_semantic;
pub mod causm_std_core;
pub mod causm_stdlib_cache;
pub mod causm_sync;
pub mod causm_syntax_ergonomics;
pub mod causm_temporal;
pub mod causm_temporal_contracts;
pub mod causm_vfs;
pub mod causm_z3_routine_test;
pub mod causm_z3_verification;

pub fn run_with_timeout<F, T>(timeout: std::time::Duration, f: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let res = f();
        let _ = tx.send(res);
    });
    rx.recv_timeout(timeout)
        .expect("Test execution timed out: runtime guard failed to terminate within time limit")
}
