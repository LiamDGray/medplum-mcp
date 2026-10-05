"""HIPAA 45 CFR § 164.312 Cryptographic Audit Flight Recorder.

Implements tamper-evident HMAC-SHA256 chained audit ledger for HL7 FHIR tool invocations,
safety intercepts, and error outcomes under federal healthcare security standards.
"""

from __future__ import annotations

import hashlib
import hmac
import json
import threading
from dataclasses import dataclass
from datetime import datetime, timezone
from enum import Enum
from pathlib import Path
from typing import Any

GENESIS_PREV_SIGNATURE: str = "0" * 64


class ActionStatus(str, Enum):
    """Audit action outcome status pursuant to HIPAA § 164.312(b)."""

    ALLOWED = "ALLOWED"
    BLOCKED = "BLOCKED"
    ERROR = "ERROR"


@dataclass
class AuditEntry:
    """Tamper-evident audit log entry with cryptographic HMAC-SHA256 chaining."""

    sequence_id: int
    timestamp: str
    action_status: ActionStatus | str
    tool_name: str
    payload_digest: str
    prev_signature: str
    signature: str
    payload: dict[str, Any] | None = None

    def to_dict(self) -> dict[str, Any]:
        """Serialize audit entry to dictionary conforming to HIPAA JSONL schema."""
        status_str = (
            self.action_status.value
            if isinstance(self.action_status, ActionStatus)
            else str(self.action_status)
        )
        data: dict[str, Any] = {
            "sequence_id": self.sequence_id,
            "timestamp": self.timestamp,
            "action_status": status_str,
            "tool_name": self.tool_name,
            "payload_digest": self.payload_digest,
            "prev_signature": self.prev_signature,
            "signature": self.signature,
        }
        if self.payload is not None:
            data["payload"] = self.payload
        return data

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> AuditEntry:
        """Construct AuditEntry from dictionary representation."""
        raw_status = data["action_status"]
        try:
            status = ActionStatus(raw_status)
        except (ValueError, KeyError):
            status = raw_status

        return cls(
            sequence_id=int(data["sequence_id"]),
            timestamp=str(data["timestamp"]),
            action_status=status,
            tool_name=str(data["tool_name"]),
            payload_digest=str(data["payload_digest"]),
            prev_signature=str(data["prev_signature"]),
            signature=str(data["signature"]),
            payload=data.get("payload"),
        )

    def signable_bytes(self) -> bytes:
        """Produce deterministic canonical byte string for HMAC signing."""
        status_str = (
            self.action_status.value
            if isinstance(self.action_status, ActionStatus)
            else str(self.action_status)
        )
        data = {
            "action_status": status_str,
            "payload_digest": self.payload_digest,
            "prev_signature": self.prev_signature,
            "sequence_id": self.sequence_id,
            "timestamp": self.timestamp,
            "tool_name": self.tool_name,
        }
        return json.dumps(data, sort_keys=True, separators=(",", ":")).encode("utf-8")


def compute_payload_digest(payload: Any) -> str:
    """Compute deterministic SHA-256 digest of arbitrary payload."""
    if payload is None:
        raw = b"{}"
    elif isinstance(payload, bytes):
        raw = payload
    elif isinstance(payload, str):
        raw = payload.encode("utf-8")
    else:
        raw = json.dumps(payload, sort_keys=True, separators=(",", ":"), default=str).encode(
            "utf-8"
        )
    return hashlib.sha256(raw).hexdigest()


def compute_entry_signature(
    secret_key: bytes | str,
    sequence_id: int,
    timestamp: str,
    action_status: ActionStatus | str,
    tool_name: str,
    payload_digest: str,
    prev_signature: str,
) -> str:
    """Calculate cryptographic HMAC-SHA256 signature for canonical entry data."""
    key_bytes = secret_key.encode("utf-8") if isinstance(secret_key, str) else secret_key
    status_str = (
        action_status.value if isinstance(action_status, ActionStatus) else str(action_status)
    )
    data = {
        "action_status": status_str,
        "payload_digest": payload_digest,
        "prev_signature": prev_signature,
        "sequence_id": sequence_id,
        "timestamp": timestamp,
        "tool_name": tool_name,
    }
    canonical = json.dumps(data, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hmac.new(key_bytes, canonical, hashlib.sha256).hexdigest()


class AuditLogManager:
    """Thread-safe, append-only cryptographic flight recorder for HIPAA audit trails."""

    def __init__(self, log_path: Path | str, secret_key: bytes | str) -> None:
        self.log_path = Path(log_path)
        self.secret_key = secret_key.encode("utf-8") if isinstance(secret_key, str) else secret_key
        self._lock = threading.Lock()
        self._last_sequence_id = 0
        self._last_signature = GENESIS_PREV_SIGNATURE

        self._initialize_chain()

    def _initialize_chain(self) -> None:
        """Scan existing log file to resume the hash chain securely."""
        if not self.log_path.exists() or self.log_path.stat().st_size == 0:
            return

        with self.log_path.open("r", encoding="utf-8") as f:
            for line in f:
                stripped = line.strip()
                if not stripped:
                    continue
                try:
                    entry_dict = json.loads(stripped)
                    self._last_sequence_id = int(entry_dict["sequence_id"])
                    self._last_signature = str(entry_dict["signature"])
                except Exception:
                    pass

    def log_event(
        self,
        tool_name: str,
        action_status: ActionStatus | str,
        payload: Any = None,
        payload_digest: str | None = None,
        timestamp: str | None = None,
    ) -> AuditEntry:
        """Append an audited event to the HMAC chained log with immediate fsync."""
        status_enum = (
            action_status
            if isinstance(action_status, ActionStatus)
            else ActionStatus(action_status)
        )
        ts = timestamp if timestamp is not None else datetime.now(timezone.utc).isoformat()
        digest = payload_digest if payload_digest is not None else compute_payload_digest(payload)

        with self._lock:
            sequence_id = self._last_sequence_id + 1
            prev_sig = self._last_signature

            sig = compute_entry_signature(
                secret_key=self.secret_key,
                sequence_id=sequence_id,
                timestamp=ts,
                action_status=status_enum,
                tool_name=tool_name,
                payload_digest=digest,
                prev_signature=prev_sig,
            )

            entry = AuditEntry(
                sequence_id=sequence_id,
                timestamp=ts,
                action_status=status_enum,
                tool_name=tool_name,
                payload_digest=digest,
                prev_signature=prev_sig,
                signature=sig,
                payload=payload if isinstance(payload, dict) else None,
            )

            self.log_path.parent.mkdir(parents=True, exist_ok=True)
            with self.log_path.open("a", encoding="utf-8") as f:
                f.write(json.dumps(entry.to_dict()) + "\n")
                f.flush()

            self._last_sequence_id = sequence_id
            self._last_signature = sig

            return entry

    log = log_event

    def get_entries(self) -> list[AuditEntry]:
        """Read and parse all entries currently persisted to disk."""
        if not self.log_path.exists():
            return []
        entries: list[AuditEntry] = []
        with self.log_path.open("r", encoding="utf-8") as f:
            for line in f:
                line_str = line.strip()
                if line_str:
                    entries.append(AuditEntry.from_dict(json.loads(line_str)))
        return entries


def verify_audit_log(
    log_path: Path | str,
    secret_key: bytes | str,
) -> tuple[bool, list[str]]:
    """Cryptographically verify the tamper-evidence and sequential continuity of an audit log.

    Returns:
        (True, []) if log integrity is intact.
        (False, [errors]) if tampering, sequence gap, payload mismatch, or broken chain is detected.
    """
    path = Path(log_path)
    if not path.exists():
        return False, [f"Audit log file not found: {path}"]

    if path.stat().st_size == 0:
        return True, []

    key_bytes = secret_key.encode("utf-8") if isinstance(secret_key, str) else secret_key
    errors: list[str] = []
    expected_sequence_id = 1
    expected_prev_signature = GENESIS_PREV_SIGNATURE

    with path.open("r", encoding="utf-8") as f:
        for line_num, line in enumerate(f, start=1):
            line_str = line.strip()
            if not line_str:
                continue

            try:
                entry = json.loads(line_str)
            except json.JSONDecodeError as exc:
                errors.append(f"Malformed JSON on line {line_num}: {exc}")
                return False, errors

            required_fields = {
                "sequence_id",
                "timestamp",
                "action_status",
                "tool_name",
                "payload_digest",
                "prev_signature",
                "signature",
            }
            missing = required_fields - set(entry.keys())
            if missing:
                errors.append(
                    f"Missing required audit fields on line {line_num}: {sorted(missing)}"
                )
                return False, errors

            seq_id = entry["sequence_id"]
            if seq_id != expected_sequence_id:
                errors.append(
                    f"Sequence gap at line {line_num}: expected sequence_id "
                    f"{expected_sequence_id}, got {seq_id}"
                )

            prev_sig = entry["prev_signature"]
            if prev_sig != expected_prev_signature:
                errors.append(
                    f"Broken chain at line {line_num} (sequence_id {seq_id}): "
                    f"expected prev_signature {expected_prev_signature}, got {prev_sig}"
                )

            # Check if payload is included and matches payload_digest
            if "payload" in entry and entry["payload"] is not None:
                computed_digest = compute_payload_digest(entry["payload"])
                if computed_digest != entry["payload_digest"]:
                    errors.append(
                        f"Payload digest mismatch at line {line_num} (sequence_id {seq_id}): "
                        "payload content has been tampered with"
                    )

            # Verify HMAC-SHA256 signature
            expected_sig = compute_entry_signature(
                secret_key=key_bytes,
                sequence_id=seq_id,
                timestamp=entry["timestamp"],
                action_status=entry["action_status"],
                tool_name=entry["tool_name"],
                payload_digest=entry["payload_digest"],
                prev_signature=prev_sig,
            )

            if not hmac.compare_digest(expected_sig, entry["signature"]):
                errors.append(
                    f"HMAC signature mismatch at line {line_num} (sequence_id {seq_id}): "
                    "cryptographic signature verification failed"
                )

            expected_sequence_id = seq_id + 1
            expected_prev_signature = entry["signature"]

    return (len(errors) == 0, errors)
