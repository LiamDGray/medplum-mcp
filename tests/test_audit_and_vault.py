"""HIPAA 45 CFR § 164.312 Audit Flight Recorder and Multi-Vault Tests."""

import json
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from medplum_mcp.audit import (
    GENESIS_PREV_SIGNATURE,
    ActionStatus,
    AuditEntry,
    AuditLogManager,
    compute_entry_signature,
    compute_payload_digest,
    verify_audit_log,
)
from medplum_mcp.vault import (
    SecretString,
    VaultResolutionError,
    resolve_secret,
)


class TestAuditFlightRecorder:
    """HIPAA § 164.312(b) Audit Controls & Cryptographic Ledger Verification."""

    def test_action_status_enum(self) -> None:
        """Verify ActionStatus enum values."""
        assert ActionStatus.ALLOWED == "ALLOWED"
        assert ActionStatus.BLOCKED == "BLOCKED"
        assert ActionStatus.ERROR == "ERROR"
        assert ActionStatus("ALLOWED") is ActionStatus.ALLOWED
        assert ActionStatus("BLOCKED") is ActionStatus.BLOCKED
        assert ActionStatus("ERROR") is ActionStatus.ERROR

    def test_genesis_prev_signature_constant(self) -> None:
        """Verify genesis signature anchor is 64 zeros (SHA-256 length)."""
        assert GENESIS_PREV_SIGNATURE == "0" * 64
        assert len(GENESIS_PREV_SIGNATURE) == 64

    def test_compute_payload_digest_and_entry_signature(self) -> None:
        """Verify deterministic payload digest and HMAC-SHA256 signature computation."""
        digest = compute_payload_digest({"key": "val"})
        assert len(digest) == 64
        assert compute_payload_digest(None) == compute_payload_digest(None)

        sig = compute_entry_signature(
            secret_key=b"k",
            sequence_id=1,
            timestamp="2026-10-04T00:00:00Z",
            action_status=ActionStatus.ALLOWED,
            tool_name="test_tool",
            payload_digest=digest,
            prev_signature=GENESIS_PREV_SIGNATURE,
        )
        assert len(sig) == 64

    def test_audit_entry_fields_and_serialization(self) -> None:
        """Verify AuditEntry fields, to_dict, and from_dict."""
        expected_digest = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        entry = AuditEntry(
            sequence_id=1,
            timestamp="2026-10-04T12:00:00Z",
            action_status=ActionStatus.ALLOWED,
            tool_name="create_medication_request",
            payload_digest=expected_digest,
            prev_signature=GENESIS_PREV_SIGNATURE,
            signature="a" * 64,
            payload={"resourceType": "MedicationRequest", "status": "draft"},
        )

        assert entry.sequence_id == 1
        assert entry.timestamp == "2026-10-04T12:00:00Z"
        assert entry.action_status == ActionStatus.ALLOWED
        assert entry.tool_name == "create_medication_request"
        assert entry.payload_digest == expected_digest
        assert entry.prev_signature == GENESIS_PREV_SIGNATURE
        assert entry.signature == "a" * 64
        assert entry.payload == {"resourceType": "MedicationRequest", "status": "draft"}

        as_dict = entry.to_dict()
        assert as_dict["sequence_id"] == 1
        assert as_dict["action_status"] == "ALLOWED"
        assert as_dict["payload"] == {"resourceType": "MedicationRequest", "status": "draft"}

        reconstructed = AuditEntry.from_dict(as_dict)
        assert reconstructed.sequence_id == entry.sequence_id
        assert reconstructed.timestamp == entry.timestamp
        assert reconstructed.action_status == entry.action_status
        assert reconstructed.tool_name == entry.tool_name
        assert reconstructed.payload_digest == entry.payload_digest
        assert reconstructed.prev_signature == entry.prev_signature
        assert reconstructed.signature == entry.signature
        assert reconstructed.payload == entry.payload

    def test_audit_log_manager_append_and_verify_clean(self, tmp_path: Path) -> None:
        """Verify AuditLogManager appends entries and verify_audit_log passes for untampered log."""
        log_file = tmp_path / "hipaa_audit.jsonl"
        secret_key = b"hipaa_production_test_hmac_secret_key"

        manager = AuditLogManager(log_path=log_file, secret_key=secret_key)

        entry1 = manager.log_event(
            tool_name="read_patient",
            action_status=ActionStatus.ALLOWED,
            payload={"patient_id": "p-101"},
        )
        entry2 = manager.log_event(
            tool_name="create_observation",
            action_status=ActionStatus.BLOCKED,
            payload={"resourceType": "Observation", "status": "final"},
        )
        entry3 = manager.log_event(
            tool_name="update_diagnostic_report",
            action_status=ActionStatus.ERROR,
            payload={"error": "Network timeout to FHIR server"},
        )

        assert entry1.sequence_id == 1
        assert entry1.prev_signature == GENESIS_PREV_SIGNATURE
        assert entry2.sequence_id == 2
        assert entry2.prev_signature == entry1.signature
        assert entry3.sequence_id == 3
        assert entry3.prev_signature == entry2.signature

        # Verify on disk
        raw_lines = log_file.read_text(encoding="utf-8").splitlines()
        lines = [line.strip() for line in raw_lines if line.strip()]
        assert len(lines) == 3

        # Verify cryptographic integrity
        valid, errors = verify_audit_log(log_path=log_file, secret_key=secret_key)
        assert valid is True
        assert errors == []

    def test_verify_detects_bit_flip_signature(self, tmp_path: Path) -> None:
        """Detect bit-flips in signature."""
        log_file = tmp_path / "audit.jsonl"
        secret_key = b"test_secret"
        manager = AuditLogManager(log_path=log_file, secret_key=secret_key)

        manager.log_event(tool_name="tool_a", action_status=ActionStatus.ALLOWED, payload={"x": 1})
        manager.log_event(tool_name="tool_b", action_status=ActionStatus.ALLOWED, payload={"x": 2})

        lines = log_file.read_text(encoding="utf-8").splitlines()
        entry1 = json.loads(lines[0])
        # Flip last character of signature
        orig_sig = entry1["signature"]
        flipped_sig = orig_sig[:-1] + ("0" if orig_sig[-1] != "0" else "1")
        entry1["signature"] = flipped_sig
        lines[0] = json.dumps(entry1)
        log_file.write_text("\n".join(lines) + "\n", encoding="utf-8")

        valid, errors = verify_audit_log(log_path=log_file, secret_key=secret_key)
        assert valid is False
        assert len(errors) > 0
        assert any("signature" in e.lower() for e in errors)

    def test_verify_detects_modified_payload(self, tmp_path: Path) -> None:
        """Detect modified payload inside audit log."""
        log_file = tmp_path / "audit.jsonl"
        secret_key = b"test_secret"
        manager = AuditLogManager(log_path=log_file, secret_key=secret_key)

        manager.log_event(
            tool_name="create_claim",
            action_status=ActionStatus.ALLOWED,
            payload={"resourceType": "Claim", "status": "draft", "amount": 100},
        )

        lines = log_file.read_text(encoding="utf-8").splitlines()
        entry = json.loads(lines[0])
        entry["payload"]["amount"] = 999999  # Tamper payload
        lines[0] = json.dumps(entry)
        log_file.write_text("\n".join(lines) + "\n", encoding="utf-8")

        valid, errors = verify_audit_log(log_path=log_file, secret_key=secret_key)
        assert valid is False
        assert len(errors) > 0
        assert any("payload" in e.lower() for e in errors)

    def test_verify_detects_altered_timestamp(self, tmp_path: Path) -> None:
        """Detect modified timestamp in audit log entry."""
        log_file = tmp_path / "audit.jsonl"
        secret_key = b"test_secret"
        manager = AuditLogManager(log_path=log_file, secret_key=secret_key)

        manager.log_event(tool_name="tool_a", action_status=ActionStatus.ALLOWED)

        lines = log_file.read_text(encoding="utf-8").splitlines()
        entry = json.loads(lines[0])
        entry["timestamp"] = "1999-01-01T00:00:00Z"  # Backdated timestamp
        lines[0] = json.dumps(entry)
        log_file.write_text("\n".join(lines) + "\n", encoding="utf-8")

        valid, errors = verify_audit_log(log_path=log_file, secret_key=secret_key)
        assert valid is False
        assert len(errors) > 0

    def test_verify_detects_reordered_entries(self, tmp_path: Path) -> None:
        """Detect out-of-order or reordered audit log entries."""
        log_file = tmp_path / "audit.jsonl"
        secret_key = b"test_secret"
        manager = AuditLogManager(log_path=log_file, secret_key=secret_key)

        manager.log_event(tool_name="step_1", action_status=ActionStatus.ALLOWED)
        manager.log_event(tool_name="step_2", action_status=ActionStatus.ALLOWED)

        lines = log_file.read_text(encoding="utf-8").splitlines()
        # Swap entry 1 and entry 2
        lines[0], lines[1] = lines[1], lines[0]
        log_file.write_text("\n".join(lines) + "\n", encoding="utf-8")

        valid, errors = verify_audit_log(log_path=log_file, secret_key=secret_key)
        assert valid is False
        assert len(errors) > 0
        assert any("sequence" in e.lower() or "chain" in e.lower() for e in errors)

    def test_verify_detects_broken_hash_chain(self, tmp_path: Path) -> None:
        """Detect broken prev_signature chain pointer."""
        log_file = tmp_path / "audit.jsonl"
        secret_key = b"test_secret"
        manager = AuditLogManager(log_path=log_file, secret_key=secret_key)

        manager.log_event(tool_name="tool_a", action_status=ActionStatus.ALLOWED)
        manager.log_event(tool_name="tool_b", action_status=ActionStatus.ALLOWED)

        lines = log_file.read_text(encoding="utf-8").splitlines()
        entry2 = json.loads(lines[1])
        entry2["prev_signature"] = "f" * 64
        lines[1] = json.dumps(entry2)
        log_file.write_text("\n".join(lines) + "\n", encoding="utf-8")

        valid, errors = verify_audit_log(log_path=log_file, secret_key=secret_key)
        assert valid is False
        assert len(errors) > 0
        assert any("chain" in e.lower() or "prev_signature" in e.lower() for e in errors)

    def test_reopening_existing_log_preserves_chain(self, tmp_path: Path) -> None:
        """Manager restarted from existing file continues sequence and chaining seamlessly."""
        log_file = tmp_path / "continuous_audit.jsonl"
        secret_key = b"continuous_secret"

        mgr1 = AuditLogManager(log_path=log_file, secret_key=secret_key)
        e1 = mgr1.log_event(tool_name="op1", action_status=ActionStatus.ALLOWED)
        e2 = mgr1.log_event(tool_name="op2", action_status=ActionStatus.ALLOWED)
        assert e1.sequence_id == 1
        assert e2.sequence_id == 2

        # Start a new instance pointing to same file
        mgr2 = AuditLogManager(log_path=log_file, secret_key=secret_key)
        e3 = mgr2.log_event(tool_name="op3", action_status=ActionStatus.ALLOWED)
        assert e3.sequence_id == 3
        assert e3.prev_signature == e2.signature

        valid, errors = verify_audit_log(log_path=log_file, secret_key=secret_key)
        assert valid is True
        assert errors == []

    def test_verify_empty_and_nonexistent_log(self, tmp_path: Path) -> None:
        """Empty log is valid (vacuously), nonexistent log returns error."""
        empty_log = tmp_path / "empty.jsonl"
        empty_log.touch()
        valid, errors = verify_audit_log(log_path=empty_log, secret_key=b"k")
        assert valid is True
        assert errors == []

        missing_log = tmp_path / "nonexistent.jsonl"
        valid, errors = verify_audit_log(log_path=missing_log, secret_key=b"k")
        assert valid is False
        assert len(errors) > 0
        assert any("not found" in e.lower() or "does not exist" in e.lower() for e in errors)


class TestZeroLeakMultiVaultResolver:
    """Zero-Trust Multi-Vault Credential Resolver & Secret Isolation."""

    def test_secret_string_masking(self) -> None:
        """SecretString representation and string formatting must never reveal value."""
        secret = SecretString("super_secret_phi_encryption_key")
        assert repr(secret) == "***"
        assert str(secret) == "***"
        assert f"{secret}" == "***"
        assert "%s" % secret == "***"
        assert secret.get_secret_value() == "super_secret_phi_encryption_key"
        assert secret.expose() == "super_secret_phi_encryption_key"

    def test_secret_string_equality_and_comparison(self) -> None:
        """SecretString implements constant-time equality."""
        s1 = SecretString("token_alpha")
        s2 = SecretString("token_alpha")
        s3 = SecretString("token_beta")

        assert s1 == s2
        assert s1 == "token_alpha"
        assert s1 != s3
        assert s1 != "token_beta"
        assert s1 != 12345
        assert bool(s1) is True
        assert bool(SecretString("")) is False
        assert len(s1) == len("token_alpha")

    def test_resolve_environment_variable(self, monkeypatch: pytest.MonkeyPatch) -> None:
        """Resolves standard environment variable (bare name or env: prefix)."""
        monkeypatch.setenv("MEDPLUM_CLIENT_SECRET", "super_medplum_secret_123")

        res1 = resolve_secret("MEDPLUM_CLIENT_SECRET")
        assert isinstance(res1, SecretString)
        assert res1.get_secret_value() == "super_medplum_secret_123"

        res2 = resolve_secret("env:MEDPLUM_CLIENT_SECRET")
        assert isinstance(res2, SecretString)
        assert res2.get_secret_value() == "super_medplum_secret_123"

    def test_resolve_1password_cli_mocked(self) -> None:
        """Resolves op:// URI using mocked 1Password CLI."""
        mock_proc = MagicMock()
        mock_proc.returncode = 0
        mock_proc.stdout = "op_retrieved_fhir_token\n"
        mock_proc.stderr = ""

        with patch("subprocess.run", return_value=mock_proc) as mock_run:
            secret = resolve_secret("op://HealthcareVault/Medplum/client_secret")
            assert isinstance(secret, SecretString)
            assert secret.get_secret_value() == "op_retrieved_fhir_token"
            mock_run.assert_called_once_with(
                ["op", "read", "op://HealthcareVault/Medplum/client_secret"],
                capture_output=True,
                text=True,
                check=False,
            )

    def test_resolve_1password_cli_failure(self) -> None:
        """Fails cleanly with VaultResolutionError when op CLI returns error."""
        mock_proc = MagicMock()
        mock_proc.returncode = 1
        mock_proc.stderr = "Item not found in vault"

        with patch("subprocess.run", return_value=mock_proc):
            with pytest.raises(VaultResolutionError, match="1Password resolution failed"):
                resolve_secret("op://HealthcareVault/MissingItem/token")

    def test_resolve_aws_secrets_manager_arn_and_key(self) -> None:
        """Resolves arn:aws:secretsmanager:... URI with JSON key extraction."""
        mock_boto_client = MagicMock()
        mock_boto_client.get_secret_value.return_value = {
            "SecretString": json.dumps({"client_secret": "aws_fhir_token_xyz"}),
        }

        mock_boto3 = MagicMock()
        mock_boto3.client.return_value = mock_boto_client

        with patch.dict("sys.modules", {"boto3": mock_boto3}):
            arn_uri = (
                "arn:aws:secretsmanager:us-east-1:123456789012:secret:"
                "medplum/prod-abcdef:client_secret"
            )
            secret = resolve_secret(arn_uri)
            assert isinstance(secret, SecretString)
            assert secret.get_secret_value() == "aws_fhir_token_xyz"
            mock_boto_client.get_secret_value.assert_called_once_with(
                SecretId="arn:aws:secretsmanager:us-east-1:123456789012:secret:medplum/prod-abcdef"
            )

    def test_resolve_aws_secrets_manager_raw_string(self) -> None:
        """Resolves arn:aws:secretsmanager:... URI when secret is raw string."""
        mock_boto_client = MagicMock()
        mock_boto_client.get_secret_value.return_value = {
            "SecretString": "raw_unstructured_secret_token",
        }

        mock_boto3 = MagicMock()
        mock_boto3.client.return_value = mock_boto_client

        with patch.dict("sys.modules", {"boto3": mock_boto3}):
            arn_uri = "arn:aws:secretsmanager:us-east-1:123456789012:secret:medplum/raw-token"
            secret = resolve_secret(arn_uri)
            assert isinstance(secret, SecretString)
            assert secret.get_secret_value() == "raw_unstructured_secret_token"

    def test_resolve_hashicorp_vault_mocked(self, monkeypatch: pytest.MonkeyPatch) -> None:
        """Resolves vault://path#field URI with mocked HTTP call."""
        monkeypatch.setenv("VAULT_TOKEN", "test_vault_root_token")
        monkeypatch.setenv("VAULT_ADDR", "https://vault.internal.net:8200")

        mock_resp = MagicMock()
        mock_resp.status_code = 200
        mock_resp.json.return_value = {
            "data": {
                "data": {
                    "medplum_auth_secret": "hv_secret_token_999",
                }
            }
        }
        mock_resp.raise_for_status = MagicMock()

        with patch("requests.get", return_value=mock_resp) as mock_get:
            secret = resolve_secret("vault://secret/data/healthcare/medplum#medplum_auth_secret")
            assert isinstance(secret, SecretString)
            assert secret.get_secret_value() == "hv_secret_token_999"

            mock_get.assert_called_once_with(
                "https://vault.internal.net:8200/v1/secret/data/healthcare/medplum",
                headers={"X-Vault-Token": "test_vault_root_token"},
                timeout=30.0,
            )

    def test_resolve_secret_raises_on_missing_or_invalid_scheme(self) -> None:
        """Raises VaultResolutionError on nonexistent variables or unknown URI schemes."""
        # Non-existent env var
        with pytest.raises(VaultResolutionError, match="not found in environment"):
            resolve_secret("COMPLETELY_NON_EXISTENT_VAR_12345")

        # Unsupported protocol scheme
        with pytest.raises(VaultResolutionError, match="Unsupported"):
            resolve_secret("unknown-vault://secret/item")

        with pytest.raises(VaultResolutionError, match="Unsupported"):
            resolve_secret("ftp://secrets.example.com/key")
