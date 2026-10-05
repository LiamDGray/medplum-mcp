//! High-Assurance Operator Safety Interlock and Cryptographic Witness Typestates.
//!
//! Enforces affine (linear consumption) typestates for industrial Modbus write operations.
//! Modbus controllers operate physical machinery (actuators, heaters, valves); writes
//! are strictly locked by default and cannot proceed without a cryptographically verified
//! supervisor witness token.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::marker::PhantomData;
use thiserror::Error;

use crate::registers::{ModbusError, ModbusRegisterBank};

type HmacSha256 = Hmac<Sha256>;

/// Errors occurring during safety interlock arming or execution.
#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum InterlockError {
    #[error("Supervisor cryptographic HMAC signature verification failed")]
    SignatureVerificationFailed,

    #[error("Supervisor authorization expired at {expires_at}, current time is {now}")]
    AuthorizationExpired { expires_at: u64, now: u64 },

    #[error(
        "Target register mismatch: authorized for {expected_register}, attempted {actual_register}"
    )]
    TargetMismatch {
        expected_register: u16,
        actual_register: u16,
    },

    #[error("Target value mismatch: authorized for {expected_value}, attempted {actual_value}")]
    ValueMismatch {
        expected_value: u16,
        actual_value: u16,
    },

    #[error("Modbus error during execution: {0}")]
    Modbus(#[from] ModbusError),
}

/// Cryptographic supervisor authorization witness for safety-critical writes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OperatorWitness {
    pub supervisor_id: String,
    pub badge_id: String,
    pub authorization_token: String,
    pub expires_at_epoch_ms: u64,
    pub target_register: u16,
    pub target_value: u16,
}

impl OperatorWitness {
    /// Constructs a canonical message payload used for HMAC signing.
    fn canonical_payload(
        supervisor_id: &str,
        badge_id: &str,
        target_register: u16,
        target_value: u16,
        expires_at_epoch_ms: u64,
    ) -> Vec<u8> {
        format!(
            "MODBUS_SAFE_WRITE:{}:{}:{}:{}:{}",
            supervisor_id, badge_id, target_register, target_value, expires_at_epoch_ms
        )
        .into_bytes()
    }

    /// Issues and cryptographically signs a new `OperatorWitness` using HMAC-SHA256.
    pub fn issue(
        supervisor_id: impl Into<String>,
        badge_id: impl Into<String>,
        target_register: u16,
        target_value: u16,
        valid_for_ms: u64,
        current_time_ms: u64,
        secret: &[u8],
    ) -> Self {
        let supervisor_id = supervisor_id.into();
        let badge_id = badge_id.into();
        let expires_at_epoch_ms = current_time_ms.saturating_add(valid_for_ms);

        let payload = Self::canonical_payload(
            &supervisor_id,
            &badge_id,
            target_register,
            target_value,
            expires_at_epoch_ms,
        );

        let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC can take key of any size");
        mac.update(&payload);
        let signature_bytes = mac.finalize().into_bytes();
        let authorization_token = hex::encode(signature_bytes);

        Self {
            supervisor_id,
            badge_id,
            authorization_token,
            expires_at_epoch_ms,
            target_register,
            target_value,
        }
    }

    /// Verifies the cryptographic token signature and expiration against the supervisor secret.
    pub fn verify(&self, secret: &[u8], current_time_ms: u64) -> Result<(), InterlockError> {
        if current_time_ms > self.expires_at_epoch_ms {
            return Err(InterlockError::AuthorizationExpired {
                expires_at: self.expires_at_epoch_ms,
                now: current_time_ms,
            });
        }

        let payload = Self::canonical_payload(
            &self.supervisor_id,
            &self.badge_id,
            self.target_register,
            self.target_value,
            self.expires_at_epoch_ms,
        );

        let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC can take key of any size");
        mac.update(&payload);

        let expected_bytes = match hex::decode(&self.authorization_token) {
            Ok(b) => b,
            Err(_) => return Err(InterlockError::SignatureVerificationFailed),
        };

        mac.verify_slice(&expected_bytes)
            .map_err(|_| InterlockError::SignatureVerificationFailed)
    }
}

/// Action to be executed on a Modbus controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteAction {
    WriteCoil { address: u16, value: bool },
    WriteHoldingRegister { address: u16, value: u16 },
}

impl WriteAction {
    pub fn address(&self) -> u16 {
        match self {
            Self::WriteCoil { address, .. } => *address,
            Self::WriteHoldingRegister { address, .. } => *address,
        }
    }

    pub fn value_as_u16(&self) -> u16 {
        match self {
            Self::WriteCoil { value, .. } => {
                if *value {
                    1
                } else {
                    0
                }
            }
            Self::WriteHoldingRegister { value, .. } => *value,
        }
    }
}

/// Safe typestate: Modbus write operations are locked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Safe;

/// Armed typestate: Modbus write operations are unlocked for single-use execution.
#[derive(Debug, PartialEq, Eq)]
pub struct Armed {
    authorized_register: u16,
    authorized_value: u16,
}

/// Affine Typestate Interlock for Modbus write safety.
#[derive(Debug)]
pub struct OperatorSafetyInterlock<State> {
    state: State,
    _marker: PhantomData<State>,
}

impl OperatorSafetyInterlock<Safe> {
    /// Creates a new interlock in the safe/locked state.
    pub fn new() -> Self {
        Self {
            state: Safe,
            _marker: PhantomData,
        }
    }

    /// Returns whether this interlock is armed for write execution.
    pub const fn is_armed(&self) -> bool {
        false
    }

    /// Arms the interlock using a valid, non-expired supervisor witness.
    ///
    /// Consumes the Safe interlock and returns an Armed interlock.
    pub fn arm(
        &self,
        witness: &OperatorWitness,
        secret: &[u8],
        current_time_ms: u64,
    ) -> Result<OperatorSafetyInterlock<Armed>, InterlockError> {
        witness.verify(secret, current_time_ms)?;

        Ok(OperatorSafetyInterlock {
            state: Armed {
                authorized_register: witness.target_register,
                authorized_value: witness.target_value,
            },
            _marker: PhantomData,
        })
    }
}

impl Default for OperatorSafetyInterlock<Safe> {
    fn default() -> Self {
        Self::new()
    }
}

impl OperatorSafetyInterlock<Armed> {
    /// Returns whether this interlock is armed for write execution.
    pub const fn is_armed(&self) -> bool {
        true
    }

    /// The target register authorized by the supervisor witness.
    pub fn authorized_register(&self) -> u16 {
        self.state.authorized_register
    }

    /// The target value authorized by the supervisor witness.
    pub fn authorized_value(&self) -> u16 {
        self.state.authorized_value
    }

    /// Attempts to execute the write action, returning the Armed interlock back if mismatched.
    pub fn try_execute(
        self,
        action: WriteAction,
        bank: &mut ModbusRegisterBank,
    ) -> (Self, Result<(), InterlockError>) {
        if action.address() != self.state.authorized_register {
            let err = InterlockError::TargetMismatch {
                expected_register: self.state.authorized_register,
                actual_register: action.address(),
            };
            return (self, Err(err));
        }

        if action.value_as_u16() != self.state.authorized_value {
            let err = InterlockError::ValueMismatch {
                expected_value: self.state.authorized_value,
                actual_value: action.value_as_u16(),
            };
            return (self, Err(err));
        }

        let res = match action {
            WriteAction::WriteCoil { address, value } => bank.set_coil(address, value),
            WriteAction::WriteHoldingRegister { address, value } => {
                bank.set_holding_register(address, value)
            }
        };

        match res {
            Ok(()) => (self, Ok(())),
            Err(e) => (self, Err(InterlockError::Modbus(e))),
        }
    }

    /// Executes the write action on the Modbus register bank.
    ///
    /// Consumes the Armed interlock (affine typestate: cannot be reused) and transitions
    /// back to the `Safe` interlock.
    pub fn execute_write(
        self,
        action: WriteAction,
        bank: &mut ModbusRegisterBank,
    ) -> Result<OperatorSafetyInterlock<Safe>, InterlockError> {
        if action.address() != self.state.authorized_register {
            return Err(InterlockError::TargetMismatch {
                expected_register: self.state.authorized_register,
                actual_register: action.address(),
            });
        }

        if action.value_as_u16() != self.state.authorized_value {
            return Err(InterlockError::ValueMismatch {
                expected_value: self.state.authorized_value,
                actual_value: action.value_as_u16(),
            });
        }

        match action {
            WriteAction::WriteCoil { address, value } => {
                bank.set_coil(address, value)?;
            }
            WriteAction::WriteHoldingRegister { address, value } => {
                bank.set_holding_register(address, value)?;
            }
        }

        Ok(OperatorSafetyInterlock {
            state: Safe,
            _marker: PhantomData,
        })
    }
}
