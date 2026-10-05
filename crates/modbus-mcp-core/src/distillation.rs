//! Register Map Distillation ("Token Diet") Engine.
//!
//! Maps raw 16-bit Modbus registers and coils into physical engineering units across three
//! fidelity levels (`Raw`, `Standard`, `Compact`) optimized for LLM token economy and latency.

use serde::{Deserialize, Serialize};

use crate::registers::{ModbusError, ModbusRegisterBank, RegisterKind};

/// Fidelity level for register distillation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetailLevel {
    /// Dense pipe-delimited format minimizing LLM context tokens (60-80% reduction vs Raw).
    Compact,
    /// Human-readable structured JSON format with engineering units and status.
    Standard,
    /// Verbose diagnostic payload containing raw hexadecimal bytes, PLC addresses, and metadata.
    Raw,
}

/// Data types supported by industrial Modbus register mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TagDataType {
    Bool,
    UInt16,
    Int16,
    UInt32,
    Int32,
    Float32,
}

/// Definition of an industrial sensor/actuator tag mapped to Modbus registers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagDefinition {
    pub name: String,
    pub register_kind: RegisterKind,
    pub address: u16,
    pub data_type: TagDataType,
    pub scale: f64,
    pub offset: f64,
    pub unit: String,
    pub description: String,
}

/// Converted tag measurement in physical engineering units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagMeasurement {
    pub name: String,
    pub address: u16,
    pub register_kind: RegisterKind,
    pub raw_hex: String,
    pub raw_value: f64,
    pub engineering_value: f64,
    pub unit: String,
    pub description: String,
}

impl TagDefinition {
    /// Reads and converts a tag's raw register value into engineering units.
    pub fn read_measurement(
        &self,
        bank: &ModbusRegisterBank,
    ) -> Result<TagMeasurement, ModbusError> {
        let (raw_value, raw_hex) = match self.register_kind {
            RegisterKind::Coil => {
                let b = bank.get_coil(self.address)?;
                (
                    if b { 1.0 } else { 0.0 },
                    format!("0x{:02X}", if b { 1 } else { 0 }),
                )
            }
            RegisterKind::DiscreteInput => {
                let b = bank.get_discrete_input(self.address)?;
                (
                    if b { 1.0 } else { 0.0 },
                    format!("0x{:02X}", if b { 1 } else { 0 }),
                )
            }
            RegisterKind::InputRegister => match self.data_type {
                TagDataType::Float32 => {
                    let f = bank.get_input_f32(self.address)?;
                    (f as f64, format!("0x{:08X}", f.to_bits()))
                }
                TagDataType::Int16 => {
                    let u = bank.get_input_register(self.address)?;
                    (u as i16 as f64, format!("0x{:04X}", u))
                }
                _ => {
                    let u = bank.get_input_register(self.address)?;
                    (u as f64, format!("0x{:04X}", u))
                }
            },
            RegisterKind::HoldingRegister => match self.data_type {
                TagDataType::Float32 => {
                    let f = bank.get_holding_f32(self.address)?;
                    (f as f64, format!("0x{:08X}", f.to_bits()))
                }
                TagDataType::Int16 => {
                    let u = bank.get_holding_register(self.address)?;
                    (u as i16 as f64, format!("0x{:04X}", u))
                }
                _ => {
                    let u = bank.get_holding_register(self.address)?;
                    (u as f64, format!("0x{:04X}", u))
                }
            },
        };

        let engineering_value = if self.data_type == TagDataType::Bool {
            raw_value
        } else {
            (raw_value * self.scale) + self.offset
        };

        Ok(TagMeasurement {
            name: self.name.clone(),
            address: self.address,
            register_kind: self.register_kind,
            raw_hex,
            raw_value,
            engineering_value,
            unit: self.unit.clone(),
            description: self.description.clone(),
        })
    }
}

/// Distills the register map into a string according to the requested `DetailLevel`.
pub fn distill_register_map(
    bank: &ModbusRegisterBank,
    tags: &[TagDefinition],
    level: DetailLevel,
) -> Result<String, ModbusError> {
    let measurements: Vec<TagMeasurement> = tags
        .iter()
        .map(|tag| tag.read_measurement(bank))
        .collect::<Result<_, _>>()?;

    match level {
        DetailLevel::Raw => {
            #[derive(Serialize)]
            struct RawDiagnosticDump<'a> {
                total_tags: usize,
                protocol: &'static str,
                measurements: &'a [TagMeasurement],
            }
            let dump = RawDiagnosticDump {
                total_tags: measurements.len(),
                protocol: "Modbus-RTU/TCP",
                measurements: &measurements,
            };
            serde_json::to_string_pretty(&dump)
                .map_err(|e| ModbusError::InterlockViolation(e.to_string()))
        }
        DetailLevel::Standard => {
            let mut map = serde_json::Map::new();
            for m in measurements {
                let mut tag_map = serde_json::Map::new();
                if m.register_kind.is_boolean() {
                    tag_map.insert(
                        "value".to_string(),
                        serde_json::Value::Bool(m.raw_value != 0.0),
                    );
                } else {
                    let val = (m.engineering_value * 100.0).round() / 100.0;
                    tag_map.insert(
                        "value".to_string(),
                        serde_json::Number::from_f64(val)
                            .map(serde_json::Value::Number)
                            .unwrap_or(serde_json::Value::Null),
                    );
                }
                tag_map.insert("unit".to_string(), serde_json::Value::String(m.unit));
                tag_map.insert(
                    "address".to_string(),
                    serde_json::Value::Number(m.address.into()),
                );
                map.insert(m.name, serde_json::Value::Object(tag_map));
            }
            serde_json::to_string_pretty(&serde_json::Value::Object(map))
                .map_err(|e| ModbusError::InterlockViolation(e.to_string()))
        }
        DetailLevel::Compact => {
            let mut parts = Vec::with_capacity(measurements.len());
            for m in measurements {
                if m.register_kind.is_boolean() {
                    let bit = if m.raw_value != 0.0 { "1" } else { "0" };
                    parts.push(format!("{}={}", m.name, bit));
                } else {
                    let rounded = (m.engineering_value * 100.0).round() / 100.0;
                    let num_str = if rounded.fract().abs() < 1e-6 {
                        format!("{:.0}", rounded)
                    } else {
                        format!("{}", rounded)
                    };
                    parts.push(format!("{}={}{}", m.name, num_str, m.unit));
                }
            }
            Ok(parts.join("|"))
        }
    }
}

/// Calculates character/token percentage reduction between raw and compact formats.
pub fn calculate_token_reduction(raw: &str, compact: &str) -> f64 {
    if raw.is_empty() {
        return 0.0;
    }
    let raw_len = raw.len() as f64;
    let compact_len = compact.len() as f64;
    ((raw_len - compact_len) / raw_len) * 100.0
}
