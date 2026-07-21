//! M4-PARITY-1 — Native contract parity tests.
//!
//! Validates the native app's session/transfer/verification model
//! against the canonical contract from bolt-core-sdk.
//!
//! Since the native session state machine lives in Swift (BoltBridge.swift),
//! this test validates the Rust contract validators directly — the same
//! validators that the Swift code was manually aligned to in M3.
//!
//! Authority: bolt-core-sdk/rust/bolt-app-core/contracts/parity_fixture.json

use bolt_app_core::contracts::session_contract::{
    is_transfer_allowed, is_valid_session_transition, is_valid_transfer_transition, SessionPhase,
    TransferPhase, VerificationState, SESSION_PHASES, TRANSFER_PHASES, VERIFICATION_STATES,
};

/// The canonical parity fixture (same file consumed by web tests).
const FIXTURE: &str =
    include_str!("../../../../bolt-core-sdk/rust/bolt-app-core/contracts/parity_fixture.json");

#[test]
fn fixture_session_phases_match_contract() {
    let doc: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let phases: Vec<String> = doc["session_phases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(phases.len(), SESSION_PHASES.len());
    for phase in &SESSION_PHASES {
        let name = serde_json::to_value(phase).unwrap();
        assert!(phases.contains(&name.as_str().unwrap().to_string()));
    }
}

#[test]
fn fixture_session_transitions_all_legal() {
    let doc: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let transitions = doc["session_transitions_legal"].as_array().unwrap();
    assert_eq!(transitions.len(), 9);
    for pair in transitions {
        let arr = pair.as_array().unwrap();
        let from: SessionPhase = serde_json::from_value(arr[0].clone()).unwrap();
        let to: SessionPhase = serde_json::from_value(arr[1].clone()).unwrap();
        assert!(
            is_valid_session_transition(from, to),
            "fixture declares {from:?} -> {to:?} but validator rejects it"
        );
    }
}

#[test]
fn fixture_session_transitions_exhaustive_illegal() {
    let doc: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let legal: Vec<(SessionPhase, SessionPhase)> = doc["session_transitions_legal"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pair| {
            let arr = pair.as_array().unwrap();
            (
                serde_json::from_value(arr[0].clone()).unwrap(),
                serde_json::from_value(arr[1].clone()).unwrap(),
            )
        })
        .collect();

    let mut illegal_count = 0;
    for from in &SESSION_PHASES {
        for to in &SESSION_PHASES {
            if !legal.contains(&(*from, *to)) {
                assert!(
                    !is_valid_session_transition(*from, *to),
                    "expected illegal: {from:?} -> {to:?}"
                );
                illegal_count += 1;
            }
        }
    }
    assert_eq!(illegal_count, 16); // 25 total - 9 legal = 16 illegal
}

#[test]
fn fixture_transfer_transitions_all_legal() {
    let doc: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let transitions = doc["transfer_transitions_legal"].as_array().unwrap();
    assert_eq!(transitions.len(), 10);
    for pair in transitions {
        let arr = pair.as_array().unwrap();
        let from: TransferPhase = serde_json::from_value(arr[0].clone()).unwrap();
        let to: TransferPhase = serde_json::from_value(arr[1].clone()).unwrap();
        assert!(
            is_valid_transfer_transition(from, to),
            "fixture declares {from:?} -> {to:?} but validator rejects it"
        );
    }
}

#[test]
fn fixture_transfer_gating_matches_policy() {
    let doc: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let gating = doc["transfer_gating"].as_object().unwrap();
    for (state_s, allowed_v) in gating {
        let state: VerificationState =
            serde_json::from_value(serde_json::Value::String(state_s.clone())).unwrap();
        let allowed = allowed_v.as_bool().unwrap();
        assert_eq!(
            is_transfer_allowed(true, state),
            allowed,
            "fixture: {state_s}={allowed}, validator disagrees"
        );
        // Disconnected always blocked
        assert!(!is_transfer_allowed(false, state));
    }
}

#[test]
fn fixture_verification_states_match_contract() {
    let doc: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let states: Vec<String> = doc["verification_states"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(states.len(), VERIFICATION_STATES.len());
    for state in &VERIFICATION_STATES {
        let name = serde_json::to_value(state).unwrap();
        assert!(states.contains(&name.as_str().unwrap().to_string()));
    }
}
