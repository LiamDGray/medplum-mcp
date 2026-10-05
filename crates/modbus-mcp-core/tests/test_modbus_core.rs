//! Comprehensive integration test suite for `modbus-mcp-core`.
//!
//! Enforces strict TDD across:
//! 1. Modbus register abstractions (Coils, Discrete Inputs, Input Registers, Holding Registers).
//! 2. Register map distillation ("Token Diet") with engineering units and token reduction.
//! 3. OperatorSafetyInterlock and OperatorWitness affine typestates.
//! 4. Machine event audit flight recording with zero-copy binary headers.

use modbus_mcp_core::audit::{
    BinaryAuditHeader, MachineEventKind, MachineFlightRecorder, AUDIT_MAGIC, AUDIT_VERSION,
};
use modbus_mcp_core::distillation::{
    calculate_token_reduction, distill_register_map, DetailLevel, TagDataType, TagDefinition,
};
use modbus_mcp_core::registers::{ModbusError, ModbusRegisterBank, RegisterKind, RegisterValue};
use modbus_mcp_core::safety::{
    InterlockError, OperatorSafetyInterlock, OperatorWitness, WriteAction,
};

#[test]
fn test_modbus_register_abstractions_coils_and_holding_registers() {
    let mut bank = ModbusRegisterBank::new();

    // 1. Coils (1-bit, Read/Write)
    assert!(!bank
        .get_coil(100)
        .expect("uninitialized coil returns false"));
    bank.set_coil(100, true).expect("setting coil must succeed");
    assert!(bank.get_coil(100).expect("coil must be true"));
    bank.set_coil(100, false)
        .expect("clearing coil must succeed");
    assert!(!bank.get_coil(100).expect("coil must be false"));

    // 2. Holding Registers (16-bit word, Read/Write)
    assert_eq!(
        bank.get_holding_register(500)
            .expect("uninitialized holding reg is 0"),
        0
    );
    bank.set_holding_register(500, 0xBEEF)
        .expect("setting holding register must succeed");
    assert_eq!(
        bank.get_holding_register(500)
            .expect("reading holding register must return written value"),
        0xBEEF
    );

    // Multi-register write/read float32 (IEEE 754 standard across 2 registers)
    let float_val: f32 = 123.456;
    bank.set_holding_f32(502, float_val)
        .expect("writing f32 across holding registers must succeed");
    let read_f32 = bank.get_holding_f32(502).expect("reading f32 must succeed");
    assert!((read_f32 - float_val).abs() < 0.0001);
}

#[test]
fn test_modbus_register_abstractions_read_only_invariants() {
    let mut bank = ModbusRegisterBank::new();

    // Discrete Inputs (1-bit, Read-Only for external protocol writes)
    bank.update_discrete_input_hardware(20, true);
    assert!(bank.get_discrete_input(20).expect("discrete input is true"));

    let coil_err = bank.write_register(
        RegisterKind::DiscreteInput,
        20,
        RegisterValue::DiscreteInput(false),
    );
    assert!(
        matches!(
            coil_err,
            Err(ModbusError::ReadOnlyRegister {
                kind: RegisterKind::DiscreteInput,
                address: 20
            })
        ),
        "protocol write to discrete input must return ReadOnlyRegister error"
    );

    // Input Registers (16-bit word, Read-Only for external protocol writes)
    bank.update_input_register_hardware(30, 4200);
    assert_eq!(
        bank.get_input_register(30).expect("input reg is 4200"),
        4200
    );

    let input_err = bank.write_register(
        RegisterKind::InputRegister,
        30,
        RegisterValue::InputRegister(9999),
    );
    assert!(
        matches!(
            input_err,
            Err(ModbusError::ReadOnlyRegister {
                kind: RegisterKind::InputRegister,
                address: 30
            })
        ),
        "protocol write to input register must return ReadOnlyRegister error"
    );
}

#[test]
fn test_register_map_token_diet_distillation() {
    let mut bank = ModbusRegisterBank::new();

    // Populate simulated industrial autoclave sensors
    // 1. Temperature: 121.5 °C (raw 1215 with 0.1 scale) in Input Register 100
    bank.update_input_register_hardware(100, 1215);

    // 2. Pressure: 2.15 bar (raw 215 with 0.01 scale) in Input Register 101
    bank.update_input_register_hardware(101, 215);

    // 3. Agitator RPM: 1500 RPM (raw 1500 with 1.0 scale) in Holding Register 200
    bank.set_holding_register(200, 1500).unwrap();

    // 4. Steam Valve: Active (Coil 10)
    bank.set_coil(10, true).unwrap();

    // 5. Emergency Stop: Normal/Inactive (Discrete Input 5)
    bank.update_discrete_input_hardware(5, false);

    let tags = vec![
        TagDefinition {
            name: "chamber_temp".to_string(),
            register_kind: RegisterKind::InputRegister,
            address: 100,
            data_type: TagDataType::UInt16,
            scale: 0.1,
            offset: 0.0,
            unit: "°C".to_string(),
            description: "Autoclave core sterilization chamber temperature".to_string(),
        },
        TagDefinition {
            name: "chamber_pressure".to_string(),
            register_kind: RegisterKind::InputRegister,
            address: 101,
            data_type: TagDataType::UInt16,
            scale: 0.01,
            offset: 0.0,
            unit: "bar".to_string(),
            description: "Chamber saturation steam pressure".to_string(),
        },
        TagDefinition {
            name: "agitator_speed".to_string(),
            register_kind: RegisterKind::HoldingRegister,
            address: 200,
            data_type: TagDataType::UInt16,
            scale: 1.0,
            offset: 0.0,
            unit: "RPM".to_string(),
            description: "Internal mixing agitator rotational speed".to_string(),
        },
        TagDefinition {
            name: "steam_valve".to_string(),
            register_kind: RegisterKind::Coil,
            address: 10,
            data_type: TagDataType::Bool,
            scale: 1.0,
            offset: 0.0,
            unit: "state".to_string(),
            description: "High-pressure boiler inlet valve position".to_string(),
        },
        TagDefinition {
            name: "emergency_stop".to_string(),
            register_kind: RegisterKind::DiscreteInput,
            address: 5,
            data_type: TagDataType::Bool,
            scale: 1.0,
            offset: 0.0,
            unit: "alarm".to_string(),
            description: "Physical emergency stop contact switch".to_string(),
        },
    ];

    let raw_out = distill_register_map(&bank, &tags, DetailLevel::Raw).unwrap();
    let standard_out = distill_register_map(&bank, &tags, DetailLevel::Standard).unwrap();
    let compact_out = distill_register_map(&bank, &tags, DetailLevel::Compact).unwrap();

    // Validate Raw output contains complete diagnostic metadata
    assert!(raw_out.contains("0x04BF") || raw_out.contains("1215"));
    assert!(raw_out.contains("Autoclave core sterilization chamber temperature"));

    // Validate Standard output contains clean engineering units
    assert!(standard_out.contains("121.5") && standard_out.contains("°C"));
    assert!(standard_out.contains("2.15") && standard_out.contains("bar"));
    assert!(standard_out.contains("1500") && standard_out.contains("RPM"));

    // Validate Compact output contains dense token-diet format
    assert!(compact_out.contains("chamber_temp=121.5°C"));
    assert!(compact_out.contains("chamber_pressure=2.15bar"));
    assert!(compact_out.contains("agitator_speed=1500RPM"));
    assert!(compact_out.contains("steam_valve=1"));
    assert!(compact_out.contains("emergency_stop=0"));

    // Assert significant character reduction (>65% reduction from Raw to Compact)
    let reduction = calculate_token_reduction(&raw_out, &compact_out);
    assert!(
        reduction >= 65.0,
        "Compact distillation must achieve at least 65% token/char reduction vs Raw (actual: {:.2}%)",
        reduction
    );
}

#[test]
fn test_operator_safety_interlock_and_witness_typestate() {
    let secret = b"industrial-high-assurance-supervisor-hmac-key";
    let now_epoch_ms = 1_700_000_000_000;

    let mut bank = ModbusRegisterBank::new();
    bank.set_holding_register(300, 100).unwrap();

    // 1. Initial state is Safe (affine interlock locked)
    let interlock = OperatorSafetyInterlock::new();
    assert!(!interlock.is_armed());

    // 2. Issuing a valid witness for writing 250 to holding register 300
    let valid_witness = OperatorWitness::issue(
        "supervisor-agent-07",
        "BADGE-9942",
        300,
        250,
        60_000, // 60s validity
        now_epoch_ms,
        secret,
    );

    // 3. Arming with a forged/tampered witness must fail
    let mut forged_witness = valid_witness.clone();
    forged_witness.authorization_token = "forged-token-0000".to_string();
    let arm_tampered = interlock.arm(&forged_witness, secret, now_epoch_ms);
    assert!(matches!(
        arm_tampered,
        Err(InterlockError::SignatureVerificationFailed)
    ));

    // 4. Arming with an expired witness must fail
    let arm_expired = interlock.arm(&valid_witness, secret, now_epoch_ms + 100_000);
    assert!(matches!(
        arm_expired,
        Err(InterlockError::AuthorizationExpired { .. })
    ));

    // 5. Arming with valid witness succeeds and transitions to Armed typestate
    let armed_interlock = interlock
        .arm(&valid_witness, secret, now_epoch_ms)
        .expect("arming with valid witness must succeed");
    assert!(armed_interlock.is_armed());

    // 6. Attempting an unauthorized target register fails
    let wrong_action = WriteAction::WriteHoldingRegister {
        address: 301, // Mismatched register address
        value: 250,
    };
    let (armed_returned, wrong_target_err) = armed_interlock.try_execute(wrong_action, &mut bank);
    assert!(matches!(
        wrong_target_err,
        Err(InterlockError::TargetMismatch { .. })
    ));

    // 7. Executing the authorized write consumes the Armed interlock and returns a Safe interlock
    let correct_action = WriteAction::WriteHoldingRegister {
        address: 300,
        value: 250,
    };
    let safe_interlock = armed_returned
        .execute_write(correct_action, &mut bank)
        .expect("authorized write must succeed");

    assert!(!safe_interlock.is_armed());
    assert_eq!(
        bank.get_holding_register(300).unwrap(),
        250,
        "holding register 300 must now reflect the authorized value"
    );
}

#[test]
fn test_zerocopy_binary_audit_flight_recording() {
    let secret = b"zerocopy-flight-recorder-secret-key-32b";
    let mut recorder = MachineFlightRecorder::new(secret);

    // 1. Initial flight recorder has genesis sequence 0
    assert_eq!(recorder.sequence_id(), 0);

    // 2. Record machine events
    let event1 = recorder.record_event(
        MachineEventKind::ReadRegisters,
        0,   // status: Success
        1,   // slave unit_id
        100, // address
        1215,
        b"payload-digest-chamber-temp-read",
    );

    assert_eq!(event1.sequence_id, 1);
    assert_eq!(event1.magic, AUDIT_MAGIC);
    assert_eq!(event1.version, AUDIT_VERSION);
    assert_eq!(event1.target_address, 100);
    assert_eq!(event1.raw_value, 1215);

    let event2 = recorder.record_event(
        MachineEventKind::WriteExecuted,
        0,   // status: Success
        1,   // slave unit_id
        300, // address
        250,
        b"payload-digest-speed-write-250",
    );

    assert_eq!(event2.sequence_id, 2);
    assert_eq!(event2.prev_signature, event1.signature);

    // 3. Test Zero-Copy byte transmutation
    let frame_bytes = recorder.export_frame_bytes(&event2);
    let transmuted_header = BinaryAuditHeader::from_bytes_zero_copy(&frame_bytes)
        .expect("transmutation from bytes must succeed zero-copy");
    assert_eq!(transmuted_header, event2);

    // 4. Verify cryptographic chain
    let verification = recorder.verify_chain();
    assert!(
        verification.is_ok(),
        "cryptographic flight recorder chain must verify successfully"
    );

    // 5. Tamper detection: modifying a byte in a recorded frame must fail verification
    recorder.corrupt_frame_for_test(1);
    let tampered_verification = recorder.verify_chain();
    assert!(
        tampered_verification.is_err(),
        "tampered frame must be detected by flight recorder verification"
    );
}
