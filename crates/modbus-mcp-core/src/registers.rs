//! Modbus Industrial Register Abstractions.
//!
//! Provides typed representations and safety bounds for the four fundamental Modbus register types:
//! - Coils (1-bit, Read/Write)
//! - Discrete Inputs (1-bit, Read-Only for external protocol writes)
//! - Input Registers (16-bit word, Read-Only for external protocol writes)
//! - Holding Registers (16-bit word, Read/Write)

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

/// Fundamental Modbus register types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RegisterKind {
    /// 1-bit Read/Write binary output
    Coil,
    /// 1-bit Read-Only binary input
    DiscreteInput,
    /// 16-bit Read-Only analog input
    InputRegister,
    /// 16-bit Read/Write analog output/parameter
    HoldingRegister,
}

impl RegisterKind {
    /// Returns true if this register type is read-only from the perspective of protocol writes.
    pub const fn is_read_only(&self) -> bool {
        matches!(self, Self::DiscreteInput | Self::InputRegister)
    }

    /// Returns true if this register type represents a 1-bit boolean value.
    pub const fn is_boolean(&self) -> bool {
        matches!(self, Self::Coil | Self::DiscreteInput)
    }
}

/// Typed Modbus register value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RegisterValue {
    Coil(bool),
    DiscreteInput(bool),
    InputRegister(u16),
    HoldingRegister(u16),
}

impl RegisterValue {
    /// Extracts boolean value if this is a Coil or DiscreteInput.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Coil(b) | Self::DiscreteInput(b) => Some(*b),
            _ => None,
        }
    }

    /// Extracts 16-bit unsigned integer value if this is an Input or Holding register.
    pub fn as_u16(&self) -> Option<u16> {
        match self {
            Self::InputRegister(v) | Self::HoldingRegister(v) => Some(*v),
            _ => None,
        }
    }
}

/// Errors occurring during Modbus register access or protocol operations.
#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum ModbusError {
    #[error("Address {0} is out of bounds (valid range 0..=65535)")]
    AddressOutOfBounds(u16),

    #[error("Cannot write to read-only register kind {kind:?} at address {address}")]
    ReadOnlyRegister { kind: RegisterKind, address: u16 },

    #[error("Register type mismatch")]
    TypeMismatch,

    #[error("Invalid quantity of registers requested: {count} (max {max})")]
    InvalidQuantity { count: u16, max: u16 },

    #[error("Interlock violation: {0}")]
    InterlockViolation(String),

    #[error("Cryptographic audit verification failed: {0}")]
    AuditVerificationFailed(String),
}

/// In-memory Modbus register bank representing a PLC or industrial controller state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModbusRegisterBank {
    coils: BTreeMap<u16, bool>,
    discrete_inputs: BTreeMap<u16, bool>,
    input_registers: BTreeMap<u16, u16>,
    holding_registers: BTreeMap<u16, u16>,
}

impl ModbusRegisterBank {
    /// Creates a new, empty register bank.
    pub fn new() -> Self {
        Self::default()
    }

    // --- Coils (Read/Write) ---

    /// Reads a 1-bit Coil at the given address. Defaults to `false` if unwritten.
    pub fn get_coil(&self, address: u16) -> Result<bool, ModbusError> {
        Ok(self.coils.get(&address).copied().unwrap_or(false))
    }

    /// Writes a 1-bit Coil at the given address.
    pub fn set_coil(&mut self, address: u16, value: bool) -> Result<(), ModbusError> {
        self.coils.insert(address, value);
        Ok(())
    }

    // --- Discrete Inputs (Read-Only to protocol, updated by hardware/sensors) ---

    /// Reads a 1-bit Discrete Input at the given address. Defaults to `false` if unwritten.
    pub fn get_discrete_input(&self, address: u16) -> Result<bool, ModbusError> {
        Ok(self.discrete_inputs.get(&address).copied().unwrap_or(false))
    }

    /// Simulates hardware / sensor updates to a Discrete Input (bypassing protocol read-only restriction).
    pub fn update_discrete_input_hardware(&mut self, address: u16, value: bool) {
        self.discrete_inputs.insert(address, value);
    }

    // --- Input Registers (Read-Only to protocol, updated by hardware/sensors) ---

    /// Reads a 16-bit Input Register at the given address. Defaults to `0` if unwritten.
    pub fn get_input_register(&self, address: u16) -> Result<u16, ModbusError> {
        Ok(self.input_registers.get(&address).copied().unwrap_or(0))
    }

    /// Simulates hardware / sensor updates to an Input Register.
    pub fn update_input_register_hardware(&mut self, address: u16, value: u16) {
        self.input_registers.insert(address, value);
    }

    /// Simulates hardware / sensor updates for an IEEE 754 32-bit float across 2 input registers.
    pub fn update_input_f32_hardware(&mut self, address: u16, value: f32) {
        let bits = value.to_bits();
        let high = ((bits >> 16) & 0xFFFF) as u16;
        let low = (bits & 0xFFFF) as u16;
        self.input_registers.insert(address, high);
        self.input_registers.insert(address.saturating_add(1), low);
    }

    /// Reads an IEEE 754 32-bit float across 2 input registers (Big-Endian word order).
    pub fn get_input_f32(&self, address: u16) -> Result<f32, ModbusError> {
        let high = self.get_input_register(address)?;
        let low = self.get_input_register(address.saturating_add(1))?;
        let bits = ((high as u32) << 16) | (low as u32);
        Ok(f32::from_bits(bits))
    }

    // --- Holding Registers (Read/Write) ---

    /// Reads a 16-bit Holding Register at the given address. Defaults to `0` if unwritten.
    pub fn get_holding_register(&self, address: u16) -> Result<u16, ModbusError> {
        Ok(self.holding_registers.get(&address).copied().unwrap_or(0))
    }

    /// Writes a 16-bit Holding Register at the given address.
    pub fn set_holding_register(&mut self, address: u16, value: u16) -> Result<(), ModbusError> {
        self.holding_registers.insert(address, value);
        Ok(())
    }

    /// Reads an IEEE 754 32-bit float across 2 holding registers (Big-Endian word order).
    pub fn get_holding_f32(&self, address: u16) -> Result<f32, ModbusError> {
        let high = self.get_holding_register(address)?;
        let low = self.get_holding_register(address.saturating_add(1))?;
        let bits = ((high as u32) << 16) | (low as u32);
        Ok(f32::from_bits(bits))
    }

    /// Writes an IEEE 754 32-bit float across 2 holding registers (Big-Endian word order).
    pub fn set_holding_f32(&mut self, address: u16, value: f32) -> Result<(), ModbusError> {
        let bits = value.to_bits();
        let high = ((bits >> 16) & 0xFFFF) as u16;
        let low = (bits & 0xFFFF) as u16;
        self.holding_registers.insert(address, high);
        self.holding_registers
            .insert(address.saturating_add(1), low);
        Ok(())
    }

    // --- Protocol-Enforced Write Dispatch ---

    /// Performs an external protocol write with strict invariant checking.
    ///
    /// Returns `ModbusError::ReadOnlyRegister` if trying to write to Discrete Inputs or Input Registers.
    pub fn write_register(
        &mut self,
        kind: RegisterKind,
        address: u16,
        value: RegisterValue,
    ) -> Result<(), ModbusError> {
        if kind.is_read_only() {
            return Err(ModbusError::ReadOnlyRegister { kind, address });
        }

        match (kind, value) {
            (RegisterKind::Coil, RegisterValue::Coil(b)) => self.set_coil(address, b),
            (RegisterKind::HoldingRegister, RegisterValue::HoldingRegister(v)) => {
                self.set_holding_register(address, v)
            }
            _ => Err(ModbusError::TypeMismatch),
        }
    }
}
