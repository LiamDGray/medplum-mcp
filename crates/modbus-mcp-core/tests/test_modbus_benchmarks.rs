//! Performance benchmarks and quantification suite for `modbus-mcp-core`.
//!
//! Quantifies:
//! 1. Raw Modbus registers vs Compact distillation token/byte savings.
//! 2. Zero-copy binary audit header transmutation latency.

use modbus_mcp_core::audit::{BinaryAuditHeader, MachineEventKind};
use modbus_mcp_core::distillation::{
    calculate_token_reduction, distill_register_map, DetailLevel, TagDataType, TagDefinition,
};
use modbus_mcp_core::registers::{ModbusRegisterBank, RegisterKind};
use std::time::Instant;
use zerocopy::IntoBytes;

fn create_benchmark_chemical_reactor_bank() -> (ModbusRegisterBank, Vec<TagDefinition>) {
    let mut bank = ModbusRegisterBank::new();

    let tags = vec![
        TagDefinition {
            name: "reactor_temp_c".to_string(),
            register_kind: RegisterKind::InputRegister,
            address: 100,
            data_type: TagDataType::Float32,
            scale: 1.0,
            offset: 0.0,
            unit: "C".to_string(),
            description: "Core reactor vessel internal temperature".to_string(),
        },
        TagDefinition {
            name: "jacket_temp_c".to_string(),
            register_kind: RegisterKind::InputRegister,
            address: 102,
            data_type: TagDataType::Float32,
            scale: 1.0,
            offset: 0.0,
            unit: "C".to_string(),
            description: "Cooling jacket circulating temperature".to_string(),
        },
        TagDefinition {
            name: "reactor_pressure_bar".to_string(),
            register_kind: RegisterKind::InputRegister,
            address: 104,
            data_type: TagDataType::Float32,
            scale: 1.0,
            offset: 0.0,
            unit: "bar".to_string(),
            description: "Primary reactor vessel pressure".to_string(),
        },
        TagDefinition {
            name: "feed_pump_1_speed_rpm".to_string(),
            register_kind: RegisterKind::HoldingRegister,
            address: 200,
            data_type: TagDataType::UInt16,
            scale: 1.0,
            offset: 0.0,
            unit: "rpm".to_string(),
            description: "Reagent A feed pump motor rotational speed".to_string(),
        },
        TagDefinition {
            name: "feed_pump_2_speed_rpm".to_string(),
            register_kind: RegisterKind::HoldingRegister,
            address: 201,
            data_type: TagDataType::UInt16,
            scale: 1.0,
            offset: 0.0,
            unit: "rpm".to_string(),
            description: "Reagent B feed pump motor rotational speed".to_string(),
        },
        TagDefinition {
            name: "cooling_flow_lpm".to_string(),
            register_kind: RegisterKind::InputRegister,
            address: 106,
            data_type: TagDataType::Float32,
            scale: 1.0,
            offset: 0.0,
            unit: "L/min".to_string(),
            description: "Cooling jacket coolant flow rate".to_string(),
        },
        TagDefinition {
            name: "agitator_rpm".to_string(),
            register_kind: RegisterKind::HoldingRegister,
            address: 202,
            data_type: TagDataType::UInt16,
            scale: 1.0,
            offset: 0.0,
            unit: "rpm".to_string(),
            description: "Main reactor impeller mixer speed".to_string(),
        },
        TagDefinition {
            name: "ph_level".to_string(),
            register_kind: RegisterKind::InputRegister,
            address: 108,
            data_type: TagDataType::Float32,
            scale: 1.0,
            offset: 0.0,
            unit: "pH".to_string(),
            description: "In-line chemical reactor effluent pH reading".to_string(),
        },
        TagDefinition {
            name: "cooling_valve_state".to_string(),
            register_kind: RegisterKind::Coil,
            address: 10,
            data_type: TagDataType::Bool,
            scale: 1.0,
            offset: 0.0,
            unit: "".to_string(),
            description: "Cooling water solenoid inlet valve actuator".to_string(),
        },
        TagDefinition {
            name: "emergency_quench_valve".to_string(),
            register_kind: RegisterKind::Coil,
            address: 11,
            data_type: TagDataType::Bool,
            scale: 1.0,
            offset: 0.0,
            unit: "".to_string(),
            description: "Emergency nitrogen quench dump actuator valve".to_string(),
        },
        TagDefinition {
            name: "high_temp_interlock_tripped".to_string(),
            register_kind: RegisterKind::DiscreteInput,
            address: 1,
            data_type: TagDataType::Bool,
            scale: 1.0,
            offset: 0.0,
            unit: "".to_string(),
            description: "Hardware hardwired high-temperature trip switch".to_string(),
        },
        TagDefinition {
            name: "high_pressure_interlock_tripped".to_string(),
            register_kind: RegisterKind::DiscreteInput,
            address: 2,
            data_type: TagDataType::Bool,
            scale: 1.0,
            offset: 0.0,
            unit: "".to_string(),
            description: "Hardware hardwired rupture disc pressure trip switch".to_string(),
        },
    ];

    // Populate realistic chemical reactor telemetry
    bank.update_input_f32_hardware(100, 78.45);
    bank.update_input_f32_hardware(102, 62.10);
    bank.update_input_f32_hardware(104, 3.82);
    bank.set_holding_register(200, 1450).unwrap();
    bank.set_holding_register(201, 820).unwrap();
    bank.update_input_f32_hardware(106, 45.6);
    bank.set_holding_register(202, 350).unwrap();
    bank.update_input_f32_hardware(108, 6.85);
    bank.set_coil(10, true).unwrap();
    bank.set_coil(11, false).unwrap();
    bank.update_discrete_input_hardware(1, false);
    bank.update_discrete_input_hardware(2, false);

    (bank, tags)
}

#[test]
fn test_benchmark_register_distillation_token_savings() {
    let (bank, tags) = create_benchmark_chemical_reactor_bank();

    let raw = distill_register_map(&bank, &tags, DetailLevel::Raw)
        .expect("Raw distillation must succeed");
    let standard = distill_register_map(&bank, &tags, DetailLevel::Standard)
        .expect("Standard distillation must succeed");
    let compact = distill_register_map(&bank, &tags, DetailLevel::Compact)
        .expect("Compact distillation must succeed");

    let raw_bytes = raw.len();
    let standard_bytes = standard.len();
    let compact_bytes = compact.len();

    let standard_reduction = calculate_token_reduction(&raw, &standard);
    let compact_reduction = calculate_token_reduction(&raw, &compact);

    println!("\n=== MODBUS REGISTER DISTILLATION ('TOKEN DIET') BENCHMARKS ===");
    println!("Total Industrial Sensor/Actuator Tags: {}", tags.len());
    println!("Raw Diagnostic JSON:     {:>6} bytes", raw_bytes);
    println!(
        "Standard JSON:           {:>6} bytes ({:.1}% reduction)",
        standard_bytes, standard_reduction
    );
    println!(
        "Compact Distilled:       {:>6} bytes ({:.1}% reduction)",
        compact_bytes, compact_reduction
    );
    println!("Sample Compact String:\n  {}", compact);

    // Assert that compact distillation provides > 70% reduction vs raw
    assert!(
        compact_reduction >= 70.0,
        "Compact distillation reduction ({:.1}%) must be at least 70%",
        compact_reduction
    );

    // Assert throughput: run 10,000 compact distillations and benchmark latency
    let iterations = 10_000;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = distill_register_map(&bank, &tags, DetailLevel::Compact).unwrap();
    }
    let elapsed = start.elapsed();
    let per_op_us = elapsed.as_micros() as f64 / iterations as f64;
    let ops_per_sec = (iterations as f64 / elapsed.as_secs_f64()) as u64;

    println!(
        "Distillation Throughput: {:>10} ops/sec ({:.2} µs/op)",
        ops_per_sec, per_op_us
    );
    let max_distill_us = if cfg!(debug_assertions) { 500.0 } else { 100.0 };
    assert!(
        per_op_us < max_distill_us,
        "Distillation latency ({:.2} µs) should be < {} µs",
        per_op_us,
        max_distill_us
    );
}

#[test]
fn test_benchmark_zerocopy_header_transmutation_latency() {
    let header = BinaryAuditHeader::new(
        MachineEventKind::WriteExecuted,
        0,
        1,
        200,
        1450,
        12345,
        1700000000000,
        [0xAA; 32],
        [0xBB; 32],
    );

    // Warm-up
    let bytes = header.as_bytes();
    let _ = BinaryAuditHeader::from_bytes_zero_copy(bytes).unwrap();

    let iterations = 100_000;
    let start = Instant::now();
    let mut sum: u64 = 0;
    for _ in 0..iterations {
        let raw_bytes = header.as_bytes();
        let parsed = BinaryAuditHeader::from_bytes_zero_copy(raw_bytes).unwrap();
        sum = sum.wrapping_add(parsed.sequence_id);
    }
    let elapsed = start.elapsed();

    // Prevent compiler dead-code elimination
    assert_eq!(sum, 12345 * iterations as u64);

    let per_op_ns = elapsed.as_nanos() as f64 / iterations as f64;
    let ops_per_sec = (iterations as f64 / elapsed.as_secs_f64()) as u64;

    println!("\n=== ZERO-COPY BINARY AUDIT TRANSMUTATION BENCHMARKS ===");
    println!("Frame Size:              128 bytes fixed layout (C-ABI)");
    println!("Total Iterations:        {}", iterations);
    println!("Total Elapsed:           {:.2?}", elapsed);
    println!("Transmutation Latency:   {:.2} ns/op", per_op_ns);
    println!("Transmutation Throughput:{:>10} ops/sec", ops_per_sec);

    let max_transmute_ns = if cfg!(debug_assertions) {
        5000.0
    } else {
        100.0
    };
    assert!(
        per_op_ns < max_transmute_ns,
        "Zero-copy transmutation latency ({:.2} ns) must be < {} ns",
        per_op_ns,
        max_transmute_ns
    );
}
