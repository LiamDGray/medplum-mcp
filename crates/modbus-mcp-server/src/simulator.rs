//! Virtual PLC Simulator for Chemical Reactor and Pump Station.
//!
//! Simulates physical process dynamics: reactor temperature, vessel pressure,
//! precursor fluid levels, pump run commands, agitator feedback, and emergency trip interlocks.

use std::sync::{Arc, Mutex};

use modbus_mcp_core::audit::{MachineEventKind, MachineFlightRecorder};
use modbus_mcp_core::distillation::{TagDataType, TagDefinition};
use modbus_mcp_core::registers::{ModbusError, ModbusRegisterBank, RegisterKind};
use modbus_mcp_core::safety::{
    InterlockError, OperatorSafetyInterlock, OperatorWitness, Safe, WriteAction,
};

// Coils
pub const COIL_PUMP_1: u16 = 0;
pub const COIL_PUMP_2: u16 = 1;
pub const COIL_AGITATOR: u16 = 2;
pub const COIL_HEATER: u16 = 3;
pub const COIL_SAFETY_TRIP: u16 = 4;

// Discrete Inputs
pub const DI_PUMP_1_RUNNING: u16 = 0;
pub const DI_PUMP_2_RUNNING: u16 = 1;
pub const DI_HIGH_PRESSURE_ALARM: u16 = 2;
pub const DI_HIGH_LEVEL_ALARM: u16 = 3;

// Input Registers
pub const IR_REACTOR_TEMP: u16 = 0;
pub const IR_REACTOR_PRESSURE: u16 = 1;
pub const IR_REACTOR_LEVEL: u16 = 2;
pub const IR_FLOW_RATE: u16 = 3;

// Holding Registers
pub const HR_TEMP_SETPOINT: u16 = 0;
pub const HR_PRESSURE_LIMIT: u16 = 1;
pub const HR_FLOW_SETPOINT: u16 = 2;

/// Virtual PLC Simulator simulating a chemical reactor and fluid pump station.
#[derive(Clone)]
pub struct VirtualPlcSimulator {
    bank: ModbusRegisterBank,
    tags: Vec<TagDefinition>,
    flight_recorder: Arc<Mutex<MachineFlightRecorder>>,
    secret: Vec<u8>,
    reactor_temp: u16,
    reactor_pressure: u16,
    reactor_level: u16,
    flow_rate: u16,
}

impl VirtualPlcSimulator {
    /// Constructs a new Virtual PLC Simulator initialized with physical reactor tags and audit flight recorder.
    pub fn new(secret: &[u8]) -> Self {
        let mut bank = ModbusRegisterBank::new();

        // Initial physical parameters (scaled by 10)
        let reactor_temp = 750; // 75.0 °C
        let reactor_pressure = 1013; // 101.3 kPa
        let reactor_level = 850; // 85.0 %
        let flow_rate = 0; // 0.0 L/min

        // Initialize Input Registers
        bank.update_input_register_hardware(IR_REACTOR_TEMP, reactor_temp);
        bank.update_input_register_hardware(IR_REACTOR_PRESSURE, reactor_pressure);
        bank.update_input_register_hardware(IR_REACTOR_LEVEL, reactor_level);
        bank.update_input_register_hardware(IR_FLOW_RATE, flow_rate);

        // Initialize Holding Registers (Setpoints & safety limits)
        let _ = bank.set_holding_register(HR_TEMP_SETPOINT, 750);
        let _ = bank.set_holding_register(HR_PRESSURE_LIMIT, 1500);
        let _ = bank.set_holding_register(HR_FLOW_SETPOINT, 100);

        // Initialize Discrete Inputs & Coils to false
        for addr in 0..5 {
            let _ = bank.set_coil(addr, false);
        }
        for addr in 0..4 {
            bank.update_discrete_input_hardware(addr, false);
        }

        let tags = vec![
            TagDefinition {
                name: "Reactor_Temp".to_string(),
                register_kind: RegisterKind::InputRegister,
                address: IR_REACTOR_TEMP,
                data_type: TagDataType::UInt16,
                scale: 0.1,
                offset: 0.0,
                unit: "C".to_string(),
                description: "Reactor Vessel Core Temperature".to_string(),
            },
            TagDefinition {
                name: "Reactor_Pressure".to_string(),
                register_kind: RegisterKind::InputRegister,
                address: IR_REACTOR_PRESSURE,
                data_type: TagDataType::UInt16,
                scale: 0.1,
                offset: 0.0,
                unit: "kPa".to_string(),
                description: "Reactor Vessel Pressure Sensor".to_string(),
            },
            TagDefinition {
                name: "Reactor_Level".to_string(),
                register_kind: RegisterKind::InputRegister,
                address: IR_REACTOR_LEVEL,
                data_type: TagDataType::UInt16,
                scale: 0.1,
                offset: 0.0,
                unit: "%".to_string(),
                description: "Chemical Precursor Fluid Level".to_string(),
            },
            TagDefinition {
                name: "Flow_Rate".to_string(),
                register_kind: RegisterKind::InputRegister,
                address: IR_FLOW_RATE,
                data_type: TagDataType::UInt16,
                scale: 0.1,
                offset: 0.0,
                unit: "L/min".to_string(),
                description: "Coolant and Chemical Reagent Flow Rate".to_string(),
            },
        ];

        let flight_recorder = Arc::new(Mutex::new(MachineFlightRecorder::new(secret)));

        Self {
            bank,
            tags,
            flight_recorder,
            secret: secret.to_vec(),
            reactor_temp,
            reactor_pressure,
            reactor_level,
            flow_rate,
        }
    }

    /// Access the underlying Modbus register bank.
    pub fn bank(&self) -> &ModbusRegisterBank {
        &self.bank
    }

    /// Access the underlying Modbus register bank mutably.
    pub fn bank_mut(&mut self) -> &mut ModbusRegisterBank {
        &mut self.bank
    }

    /// Access configured distillation tags.
    pub fn tags(&self) -> &[TagDefinition] {
        &self.tags
    }

    /// Access the cryptographic machine flight recorder.
    pub fn flight_recorder(&self) -> Arc<Mutex<MachineFlightRecorder>> {
        self.flight_recorder.clone()
    }

    /// Record an event into the flight recorder.
    fn record_audit(
        &self,
        kind: MachineEventKind,
        status: u8,
        target_address: u16,
        raw_val: u16,
        payload: &[u8],
    ) {
        if let Ok(mut recorder) = self.flight_recorder.lock() {
            recorder.record_event(kind, status, 1, target_address, raw_val, payload);
        }
    }

    /// Advance the reactor and pump station physics simulation by one cycle.
    pub fn step(&mut self) {
        let safety_tripped = self.bank.get_coil(COIL_SAFETY_TRIP).unwrap_or(false);

        if safety_tripped {
            let _ = self.bank.set_coil(COIL_PUMP_1, false);
            let _ = self.bank.set_coil(COIL_PUMP_2, false);
            let _ = self.bank.set_coil(COIL_HEATER, false);
            self.bank
                .update_discrete_input_hardware(DI_PUMP_1_RUNNING, false);
            self.bank
                .update_discrete_input_hardware(DI_PUMP_2_RUNNING, false);
            self.flow_rate = 0;
            self.bank.update_input_register_hardware(IR_FLOW_RATE, 0);
            return;
        }

        let pump1_cmd = self.bank.get_coil(COIL_PUMP_1).unwrap_or(false);
        let pump2_cmd = self.bank.get_coil(COIL_PUMP_2).unwrap_or(false);
        let heater_cmd = self.bank.get_coil(COIL_HEATER).unwrap_or(false);

        // Update running feedback
        self.bank
            .update_discrete_input_hardware(DI_PUMP_1_RUNNING, pump1_cmd);
        self.bank
            .update_discrete_input_hardware(DI_PUMP_2_RUNNING, pump2_cmd);

        // Flow rate simulation
        if pump1_cmd || pump2_cmd {
            let target_flow = if pump1_cmd && pump2_cmd { 240 } else { 120 };
            if self.flow_rate < target_flow {
                self.flow_rate = self.flow_rate.saturating_add(30).min(target_flow);
            }
        } else if self.flow_rate > 0 {
            self.flow_rate = self.flow_rate.saturating_sub(40);
        }
        self.bank
            .update_input_register_hardware(IR_FLOW_RATE, self.flow_rate);

        // Temperature simulation
        if heater_cmd {
            self.reactor_temp = self.reactor_temp.saturating_add(5);
        } else if self.reactor_temp > 700 {
            self.reactor_temp = self.reactor_temp.saturating_sub(1);
        }
        self.bank
            .update_input_register_hardware(IR_REACTOR_TEMP, self.reactor_temp);

        // Pressure depends on temp and level
        let pressure_calc = (1013u32 + ((self.reactor_temp as u32).saturating_sub(700) * 3)) as u16;
        self.reactor_pressure = pressure_calc;
        self.bank
            .update_input_register_hardware(IR_REACTOR_PRESSURE, self.reactor_pressure);

        // Check high pressure / high level alarms
        let pressure_limit = self
            .bank
            .get_holding_register(HR_PRESSURE_LIMIT)
            .unwrap_or(1500);
        self.bank.update_discrete_input_hardware(
            DI_HIGH_PRESSURE_ALARM,
            self.reactor_pressure >= pressure_limit,
        );
        self.bank
            .update_discrete_input_hardware(DI_HIGH_LEVEL_ALARM, self.reactor_level >= 950);
    }

    /// Triggers immediate Emergency Stop, tripping safety coil and halting pumps.
    pub fn emergency_stop(&mut self, reason: Option<&str>) {
        let msg = reason.unwrap_or("Emergency Stop triggered");
        let _ = self.bank.set_coil(COIL_SAFETY_TRIP, true);
        let _ = self.bank.set_coil(COIL_PUMP_1, false);
        let _ = self.bank.set_coil(COIL_PUMP_2, false);
        let _ = self.bank.set_coil(COIL_HEATER, false);

        self.bank
            .update_discrete_input_hardware(DI_PUMP_1_RUNNING, false);
        self.bank
            .update_discrete_input_hardware(DI_PUMP_2_RUNNING, false);
        self.flow_rate = 0;
        self.bank.update_input_register_hardware(IR_FLOW_RATE, 0);

        self.record_audit(
            MachineEventKind::InterlockTripped,
            1,
            COIL_SAFETY_TRIP,
            1,
            msg.as_bytes(),
        );
    }

    /// Reads discrete inputs from start address across `count` inputs.
    pub fn read_discrete_inputs(&self, start: u16, count: u16) -> Result<Vec<bool>, ModbusError> {
        if count == 0 || (start as u32 + count as u32) > 65536 {
            return Err(ModbusError::InvalidQuantity { count, max: 2000 });
        }
        let mut results = Vec::with_capacity(count as usize);
        for i in 0..count {
            let addr = start.saturating_add(i);
            results.push(self.bank.get_discrete_input(addr)?);
        }
        self.record_audit(
            MachineEventKind::ReadRegisters,
            1,
            start,
            count,
            b"ReadDiscreteInputs",
        );
        Ok(results)
    }

    /// Reads coils from start address across `count` coils.
    pub fn read_coils(&self, start: u16, count: u16) -> Result<Vec<bool>, ModbusError> {
        if count == 0 || (start as u32 + count as u32) > 65536 {
            return Err(ModbusError::InvalidQuantity { count, max: 2000 });
        }
        let mut results = Vec::with_capacity(count as usize);
        for i in 0..count {
            let addr = start.saturating_add(i);
            results.push(self.bank.get_coil(addr)?);
        }
        self.record_audit(
            MachineEventKind::ReadRegisters,
            1,
            start,
            count,
            b"ReadCoils",
        );
        Ok(results)
    }

    /// Reads input registers from start address across `count` registers.
    pub fn read_input_registers(&self, start: u16, count: u16) -> Result<Vec<u16>, ModbusError> {
        if count == 0 || (start as u32 + count as u32) > 65536 {
            return Err(ModbusError::InvalidQuantity { count, max: 125 });
        }
        let mut results = Vec::with_capacity(count as usize);
        for i in 0..count {
            let addr = start.saturating_add(i);
            results.push(self.bank.get_input_register(addr)?);
        }
        self.record_audit(
            MachineEventKind::ReadRegisters,
            1,
            start,
            count,
            b"ReadInputRegisters",
        );
        Ok(results)
    }

    /// Reads holding registers from start address across `count` registers.
    pub fn read_holding_registers(&self, start: u16, count: u16) -> Result<Vec<u16>, ModbusError> {
        if count == 0 || (start as u32 + count as u32) > 65536 {
            return Err(ModbusError::InvalidQuantity { count, max: 125 });
        }
        let mut results = Vec::with_capacity(count as usize);
        for i in 0..count {
            let addr = start.saturating_add(i);
            results.push(self.bank.get_holding_register(addr)?);
        }
        self.record_audit(
            MachineEventKind::ReadRegisters,
            1,
            start,
            count,
            b"ReadHoldingRegisters",
        );
        Ok(results)
    }

    /// Writes a coil under safety interlock protection with cryptographic witness validation.
    pub fn write_coil_interlocked(
        &mut self,
        address: u16,
        value: bool,
        witness: Option<&OperatorWitness>,
        now_ms: u64,
    ) -> Result<(), InterlockError> {
        let raw_val = if value { 1 } else { 0 };
        let witness = match witness {
            Some(w) => w,
            None => {
                self.record_audit(
                    MachineEventKind::WriteAttempt,
                    0,
                    address,
                    raw_val,
                    b"Blocked: Missing OperatorWitness",
                );
                return Err(InterlockError::SignatureVerificationFailed);
            }
        };

        let interlock = OperatorSafetyInterlock::<Safe>::new();
        let armed = match interlock.arm(witness, &self.secret, now_ms) {
            Ok(a) => a,
            Err(e) => {
                self.record_audit(
                    MachineEventKind::WriteAttempt,
                    0,
                    address,
                    raw_val,
                    format!("Blocked: Arm failed ({e:?})").as_bytes(),
                );
                return Err(e);
            }
        };

        match armed.execute_write(WriteAction::WriteCoil { address, value }, &mut self.bank) {
            Ok(_) => {
                self.record_audit(
                    MachineEventKind::WriteExecuted,
                    1,
                    address,
                    raw_val,
                    b"WriteCoil executed with valid witness",
                );
                Ok(())
            }
            Err(e) => {
                self.record_audit(
                    MachineEventKind::WriteAttempt,
                    0,
                    address,
                    raw_val,
                    format!("Blocked: Execution failed ({e:?})").as_bytes(),
                );
                Err(e)
            }
        }
    }

    /// Writes a holding register under safety interlock protection with cryptographic witness validation.
    pub fn write_holding_register_interlocked(
        &mut self,
        address: u16,
        value: u16,
        witness: Option<&OperatorWitness>,
        now_ms: u64,
    ) -> Result<(), InterlockError> {
        let witness = match witness {
            Some(w) => w,
            None => {
                self.record_audit(
                    MachineEventKind::WriteAttempt,
                    0,
                    address,
                    value,
                    b"Blocked: Missing OperatorWitness",
                );
                return Err(InterlockError::SignatureVerificationFailed);
            }
        };

        let interlock = OperatorSafetyInterlock::<Safe>::new();
        let armed = match interlock.arm(witness, &self.secret, now_ms) {
            Ok(a) => a,
            Err(e) => {
                self.record_audit(
                    MachineEventKind::WriteAttempt,
                    0,
                    address,
                    value,
                    format!("Blocked: Arm failed ({e:?})").as_bytes(),
                );
                return Err(e);
            }
        };

        match armed.execute_write(
            WriteAction::WriteHoldingRegister { address, value },
            &mut self.bank,
        ) {
            Ok(_) => {
                self.record_audit(
                    MachineEventKind::WriteExecuted,
                    1,
                    address,
                    value,
                    b"WriteHoldingRegister executed with valid witness",
                );
                Ok(())
            }
            Err(e) => {
                self.record_audit(
                    MachineEventKind::WriteAttempt,
                    0,
                    address,
                    value,
                    format!("Blocked: Execution failed ({e:?})").as_bytes(),
                );
                Err(e)
            }
        }
    }
}
