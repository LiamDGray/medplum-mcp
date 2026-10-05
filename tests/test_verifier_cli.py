"""Tests for Automated Cryptographic Verification CLI (medplum-mcp verify)."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest
from rich.console import Console

from medplum_mcp.audit import ActionStatus, AuditLogManager
from medplum_mcp.server import main
from medplum_mcp.verifier_cli import (
    VerificationReport,
    check_homoglyph_immunity,
    run_verification,
)


def test_run_verification_quick_success() -> None:
    """run_verification(quick=True) returns a report with all 5 clinical theorems proven."""
    report = run_verification(quick=True)

    assert isinstance(report, VerificationReport)
    assert report.success is True
    assert report.is_valid is True

    # Stage 1: Deterministic FSM Invariant Verification
    stage1 = report.stage1_smt
    assert stage1["passed"] is True
    assert stage1["proven_theorems"] == 5
    for entity in (
        "MedicationRequest",
        "AllergyIntolerance",
        "Observation",
        "DiagnosticReport",
        "Claim",
    ):
        assert entity in stage1["theorems"]
        assert stage1["theorems"][entity]["proven"] is True
        assert str(stage1["theorems"][entity]["solver_status"]).lower() in ("satisfied", "unsat")

    # Stage 2: Adversarial FSM Lifecycle & Homoglyph Immunity
    stage2 = report.stage2_fsm
    assert stage2["passed"] is True
    assert stage2["homoglyph_immunity"]["passed"] is True
    for entity in (
        "MedicationRequest",
        "AllergyIntolerance",
        "Observation",
        "DiagnosticReport",
        "Claim",
    ):
        assert stage2["lifecycles"][entity] is True

    # Stage 3: HMAC-SHA256 Cryptographic Audit Ledger
    stage3 = report.stage3_audit
    assert stage3["passed"] is True


def test_run_verification_full_depth() -> None:
    """run_verification(quick=False) uses full depth=10 for formal proofs."""
    report = run_verification(quick=False)
    assert report.success is True
    assert report.stage1_smt["theorems"]["MedicationRequest"]["max_depth"] == 10


def test_verifier_report_outputs() -> None:
    """Test serialization formats: to_dict(), to_json(), to_markdown()."""
    report = run_verification(quick=True)

    # Dict
    d = report.to_dict()
    assert d["status"] == "PASSED"
    assert d["success"] is True
    assert "stage1_reachability" in d["stages"]
    assert "stage1_smt_z3" in d["stages"]
    assert "stage2_fsm_adversarial" in d["stages"]
    assert "stage3_audit_ledger" in d["stages"]

    # JSON
    j = report.to_json()
    parsed = json.loads(j)
    assert parsed["status"] == "PASSED"

    # Markdown
    md = report.to_markdown()
    assert "# Medplum MCP Formal Verification Certificate" in md
    assert "Zero Unauthorized Commitment Invariant" in md
    assert "MedicationRequest" in md


def test_verifier_render_terminal() -> None:
    """Test rich terminal certificate rendering."""
    report = run_verification(quick=True)
    con = Console(record=True, width=120)
    report.render_terminal(console=con)
    rendered = con.export_text()

    assert "MEDPLUM MCP FORMAL VERIFICATION" in rendered
    assert "Stage 1" in rendered
    assert "Stage 2" in rendered
    assert "Stage 3" in rendered
    assert "CERTIFICATE" in rendered


def test_cli_verify_json_output(capsys: pytest.CaptureFixture[str]) -> None:
    """CLI execution medplum-mcp verify --output json prints valid JSON and returns 0."""
    exit_code = main(["verify", "--quick", "--output", "json"])
    assert exit_code == 0
    captured = capsys.readouterr()
    data: dict[str, Any] = json.loads(captured.out)

    assert data["success"] is True
    assert data["status"] == "PASSED"
    assert "stages" in data
    assert data["stages"]["stage1_reachability"]["passed"] is True
    assert data["stages"]["stage1_smt_z3"]["passed"] is True
    assert data["stages"]["stage2_fsm_adversarial"]["passed"] is True
    assert data["stages"]["stage3_audit_ledger"]["passed"] is True


def test_cli_verify_markdown_output(capsys: pytest.CaptureFixture[str]) -> None:
    """CLI execution medplum-mcp verify --output markdown prints valid markdown and returns 0."""
    exit_code = main(["verify", "--quick", "--output", "markdown"])
    assert exit_code == 0
    captured = capsys.readouterr()
    assert "# Medplum MCP Formal Verification Certificate" in captured.out


def test_cli_verify_terminal_output() -> None:
    """CLI execution medplum-mcp verify --output terminal returns 0."""
    exit_code = main(["verify", "--quick", "--output", "terminal"])
    assert exit_code == 0


def test_tampered_audit_log_strict_failure(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Tampered audit log detection: corrupted log returns exit code 1 under --strict."""
    log_path = tmp_path / "tampered_audit.jsonl"
    secret_key = b"test-secret-key-medplum"

    # Create a valid audit log first
    mgr = AuditLogManager(log_path=log_path, secret_key=secret_key)
    mgr.log_event("medplum_list_medications", ActionStatus.ALLOWED, {"patient_id": "pat-001"})
    mgr.log_event("medplum_create_medication_draft", ActionStatus.ALLOWED, {"status": "draft"})

    # Tamper with the log by corrupting line 2's payload_digest
    lines = log_path.read_text(encoding="utf-8").strip().splitlines()
    entry2 = json.loads(lines[1])
    entry2["payload_digest"] = "f" * 64
    lines[1] = json.dumps(entry2)
    log_path.write_text("\n".join(lines) + "\n", encoding="utf-8")

    # Programmatic verification should fail
    report = run_verification(
        quick=True,
        audit_log_path=log_path,
        audit_key=secret_key,
        strict=True,
    )
    assert report.success is False
    assert report.is_valid is False
    assert report.stage3_audit["passed"] is False
    assert len(report.errors) > 0

    # CLI verify under --strict must return code 1
    exit_code = main(
        [
            "verify",
            "--quick",
            "--audit-log",
            str(log_path),
            "--audit-key",
            secret_key.decode("utf-8"),
            "--strict",
            "--output",
            "json",
        ]
    )
    assert exit_code == 1


def test_valid_audit_log_verification(tmp_path: Path) -> None:
    """Valid audit log with multiple events passes all Stage 3 checks."""
    log_path = tmp_path / "valid_audit.jsonl"
    secret_key = b"valid-secret-key-123"

    mgr = AuditLogManager(log_path=log_path, secret_key=secret_key)
    mgr.log_event("medplum_search_patients", ActionStatus.ALLOWED, {"name": "Alice"})
    mgr.log_event("medplum_create_observation_draft", ActionStatus.ALLOWED, {"code": "8480-6"})
    mgr.log_event("medplum_create_medication_draft", ActionStatus.BLOCKED, {"status": "active"})

    report = run_verification(
        quick=True,
        audit_log_path=log_path,
        audit_key=secret_key,
        strict=True,
    )
    assert report.success is True
    s3 = report.stage3_audit
    assert s3["passed"] is True
    assert s3["block_height"] == 3
    assert s3["genesis_signature_valid"] is True
    assert s3["chain_valid"] is True
    assert s3["timestamps_monotonic"] is True
    assert s3["payload_digests_valid"] is True


def test_homoglyph_adversarial_vectors() -> None:
    """Direct test of check_homoglyph_immunity ensuring all evasion vectors are intercepted."""
    res = check_homoglyph_immunity()
    assert res["passed"] is True
    assert res["vectors_tested"] > 0
    assert res["vectors_blocked"] == res["vectors_tested"]
