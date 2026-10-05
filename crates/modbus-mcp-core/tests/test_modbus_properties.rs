//! Property-based testing for `modbus-mcp-core` using `proptest`.
//!
//! Verifies:
//! 1. Register IEEE 754 float32 roundtrip conversion across arbitrary floats (handling NaN/Inf).
//! 2. Sequential monotonic numbering and hash chaining across arbitrary numbers of machine events.
//! 3. Tamper detection: any bit flip in signature or payload causes verification to fail.

use modbus_mcp_core::audit::{MachineEventKind, MachineFlightRecorder};
use modbus_mcp_core::registers::ModbusRegisterBank;
use proptest::prelude::*;

proptest! {
    /// 1. IEEE 754 float32 roundtrip conversion across arbitrary floats (handling NaN/Inf).
    #[test]
    fn prop_ieee754_float32_holding_register_roundtrip(
        val in any::<f32>(),
        addr in 0u16..=65534u16,
    ) {
        let mut bank = ModbusRegisterBank::new();
        bank.set_holding_f32(addr, val).expect("set_holding_f32 must succeed");
        let read = bank.get_holding_f32(addr).expect("get_holding_f32 must succeed");

        // Exact bit-level IEEE 754 preservation across registers
        prop_assert_eq!(read.to_bits(), val.to_bits());

        if val.is_nan() {
            prop_assert!(read.is_nan());
        } else if val.is_infinite() {
            prop_assert!(read.is_infinite());
            prop_assert_eq!(read.is_sign_positive(), val.is_sign_positive());
        } else {
            prop_assert_eq!(read, val);
        }
    }

    #[test]
    fn prop_ieee754_float32_input_register_roundtrip(
        val in any::<f32>(),
        addr in 0u16..=65534u16,
    ) {
        let mut bank = ModbusRegisterBank::new();
        bank.update_input_f32_hardware(addr, val);
        let read = bank.get_input_f32(addr).expect("get_input_f32 must succeed");

        prop_assert_eq!(read.to_bits(), val.to_bits());

        if val.is_nan() {
            prop_assert!(read.is_nan());
        } else if val.is_infinite() {
            prop_assert!(read.is_infinite());
            prop_assert_eq!(read.is_sign_positive(), val.is_sign_positive());
        } else {
            prop_assert_eq!(read, val);
        }
    }

    /// 2. Sequential monotonic numbering and hash chaining across arbitrary machine events.
    #[test]
    fn prop_audit_sequential_monotonic_and_hash_chaining(
        secret in proptest::collection::vec(any::<u8>(), 16..=64),
        events in proptest::collection::vec(
            (
                prop_oneof![
                    Just(MachineEventKind::ReadRegisters),
                    Just(MachineEventKind::WriteAttempt),
                    Just(MachineEventKind::WriteExecuted),
                    Just(MachineEventKind::InterlockTripped),
                    Just(MachineEventKind::SafetyFault),
                ],
                any::<u8>(),
                any::<u8>(),
                any::<u16>(),
                any::<u16>(),
                proptest::collection::vec(any::<u8>(), 0..=128),
            ),
            1..=30
        )
    ) {
        let mut recorder = MachineFlightRecorder::new(&secret);
        let mut last_sig = [0u8; 32];

        for (idx, (kind, status, unit_id, target_addr, raw_val, payload)) in events.iter().enumerate() {
            let header = recorder.record_event(
                *kind,
                *status,
                *unit_id,
                *target_addr,
                *raw_val,
                payload,
            );

            // Monotonic sequence ID assertion
            let expected_seq = (idx + 1) as u64;
            prop_assert_eq!(header.sequence_id, expected_seq);
            prop_assert_eq!(recorder.sequence_id(), expected_seq);

            // Hash chaining assertion
            prop_assert_eq!(header.prev_signature, last_sig);

            // Self-signature verification
            prop_assert!(header.verify_signature(&secret));

            // Payload verification
            prop_assert!(header.verify_payload(payload));

            last_sig = header.signature;
        }

        // Entire chain verification must succeed
        prop_assert!(recorder.verify_chain().is_ok());
    }

    /// 3a. Tamper detection: any bit flip in signature causes verification to fail.
    #[test]
    fn prop_audit_signature_bit_flip_tamper_detection(
        secret in proptest::collection::vec(any::<u8>(), 16..=64),
        num_events in 1usize..=10usize,
        tamper_frame_idx in 0usize..10usize,
        byte_idx in 0usize..32usize,
        bit_idx in 0u8..8u8,
    ) {
        let mut recorder = MachineFlightRecorder::new(&secret);
        for i in 0..num_events {
            recorder.record_event(
                MachineEventKind::WriteExecuted,
                0,
                1,
                100 + i as u16,
                i as u16,
                b"sample_payload",
            );
        }

        let target_idx = tamper_frame_idx % num_events;

        // Flip bit in the recorder's frame signature
        recorder.flip_signature_bit_for_test(target_idx, byte_idx, bit_idx);

        // Frame signature verification must fail
        prop_assert!(!recorder.frames()[target_idx].verify_signature(&secret));

        // Full chain verification must fail
        prop_assert!(recorder.verify_chain().is_err());
    }

    /// 3b. Tamper detection: any bit flip in prev_signature causes verification to fail.
    #[test]
    fn prop_audit_prev_signature_bit_flip_tamper_detection(
        secret in proptest::collection::vec(any::<u8>(), 16..=64),
        num_events in 2usize..=10usize,
        tamper_frame_idx in 1usize..10usize,
        byte_idx in 0usize..32usize,
        bit_idx in 0u8..8u8,
    ) {
        let mut recorder = MachineFlightRecorder::new(&secret);
        for i in 0..num_events {
            recorder.record_event(
                MachineEventKind::WriteExecuted,
                0,
                1,
                100 + i as u16,
                i as u16,
                b"sample_payload",
            );
        }

        let target_idx = (tamper_frame_idx % (num_events - 1)) + 1;

        // Flip bit in prev_signature
        recorder.flip_prev_signature_bit_for_test(target_idx, byte_idx, bit_idx);

        // Frame signature verification fails because prev_signature is included in HMAC
        prop_assert!(!recorder.frames()[target_idx].verify_signature(&secret));

        // Full chain verification must fail
        prop_assert!(recorder.verify_chain().is_err());
    }

    /// 3c. Tamper detection: any bit flip in payload causes verification to fail.
    #[test]
    fn prop_audit_payload_bit_flip_tamper_detection(
        secret in proptest::collection::vec(any::<u8>(), 16..=64),
        payload in proptest::collection::vec(any::<u8>(), 1..=128),
        byte_idx in any::<usize>(),
        bit_idx in 0u8..8u8,
    ) {
        let mut recorder = MachineFlightRecorder::new(&secret);
        let header = recorder.record_event(
            MachineEventKind::WriteExecuted,
            0,
            1,
            200,
            42,
            &payload,
        );

        // Untampered payload verifies
        prop_assert!(header.verify_payload(&payload));

        // Tampered payload with 1 bit flipped
        let mut tampered_payload = payload.clone();
        let target_byte = byte_idx % tampered_payload.len();
        tampered_payload[target_byte] ^= 1 << bit_idx;

        // Verification must strictly fail
        prop_assert!(!header.verify_payload(&tampered_payload));
    }
}
