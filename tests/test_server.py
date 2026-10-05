"""Test suite for FastMCP 2.3+ Clinical Server (medplum_mcp.server).

Strict TDD suite testing AuditedMCPServer, all 16 clinical tools, 4 native FHIR resources,
2 native prompts, audit flight recorder integration, and CLI entrypoint.
"""

from __future__ import annotations

import json
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest
from mcp.types import GetPromptResult, TextContent

from medplum_mcp.audit import ActionStatus, AuditLogManager
from medplum_mcp.safety import SafetyInvariantViolation
from medplum_mcp.server import (
    AuditedMCPServer,
    build_parser,
    create_parser,
    create_server,
    main,
)


@pytest.fixture
def audit_log_path(tmp_path: Path) -> Path:
    return tmp_path / "test_audit.jsonl"


@pytest.fixture
def audit_manager(audit_log_path: Path) -> AuditLogManager:
    return AuditLogManager(log_path=audit_log_path, secret_key="test-secret-hmac-key")


# ===========================================================================
# 1. AuditedMCPServer & 16 Clinical Query Tools
# ===========================================================================


class TestAuditedMCPServerQueryTools:
    """Test read-only clinical tool execution through AuditedMCPServer."""

    @pytest.mark.anyio
    async def test_server_instance_and_tool_registration(
        self, audit_manager: AuditLogManager
    ) -> None:
        """Verify server initializes with AuditedMCPServer and registers all 16 clinical tools."""
        server = create_server(demo_mode=True, allow_writes=False, audit_manager=audit_manager)
        assert isinstance(server, AuditedMCPServer)

        expected_tools = {
            "medplum_search_patients",
            "medplum_get_patient",
            "medplum_list_observations",
            "medplum_get_observation",
            "medplum_create_observation_draft",
            "medplum_list_conditions",
            "medplum_get_condition",
            "medplum_list_medication_requests",
            "medplum_get_medication_request",
            "medplum_create_medication_draft",
            "medplum_list_allergy_intolerances",
            "medplum_get_allergy_intolerance",
            "medplum_list_diagnostic_reports",
            "medplum_get_diagnostic_report",
            "medplum_list_encounters",
            "medplum_list_care_plans",
        }

        # Check registered tools
        tools = await server.list_tools()
        registered_tool_names = {t.name for t in tools}
        for tool_name in expected_tools:
            assert tool_name in registered_tool_names, f"Tool {tool_name} not registered"

    @pytest.mark.anyio
    async def test_patient_search_and_get(self, audit_manager: AuditLogManager) -> None:
        """Test medplum_search_patients and medplum_get_patient with audit logging."""
        server = create_server(demo_mode=True, allow_writes=False, audit_manager=audit_manager)

        # Search patients
        res = await server.call_tool("medplum_search_patients", {"name": "Lin"})
        assert not res.is_error
        assert len(res.content) > 0
        content_json = json.loads(res.content[0].text)
        assert content_json.get("resourceType") == "Bundle"
        assert len(content_json.get("entry", [])) >= 1

        # Get patient
        pat_id = content_json["entry"][0]["resource"]["id"]
        res_pat = await server.call_tool("medplum_get_patient", {"patient_id": pat_id})
        assert not res_pat.is_error
        pat_json = json.loads(res_pat.content[0].text)
        assert pat_json["id"] == pat_id

        # Check audit trail recorded ALLOWED for both calls
        entries = audit_manager.get_entries()
        assert len(entries) >= 2
        assert entries[-2].tool_name == "medplum_search_patients"
        assert entries[-2].action_status == ActionStatus.ALLOWED
        assert entries[-1].tool_name == "medplum_get_patient"
        assert entries[-1].action_status == ActionStatus.ALLOWED

    @pytest.mark.anyio
    async def test_patient_search_with_dob_and_identifier(
        self, audit_manager: AuditLogManager
    ) -> None:
        """Test medplum_search_patients with identifier and dob filters."""
        server = create_server(demo_mode=True, allow_writes=False, audit_manager=audit_manager)

        res = await server.call_tool(
            "medplum_search_patients",
            {"identifier": "MRN-SJ-100235", "dob": "2015-08-20"},
        )
        assert not res.is_error
        bundle = json.loads(res.content[0].text)
        assert bundle["resourceType"] == "Bundle"
        assert len(bundle.get("entry", [])) >= 1

    @pytest.mark.anyio
    async def test_observation_tools(self, audit_manager: AuditLogManager) -> None:
        """Test medplum_list_observations and medplum_get_observation."""
        server = create_server(demo_mode=True, allow_writes=False, audit_manager=audit_manager)

        # List observations
        res = await server.call_tool(
            "medplum_list_observations",
            {"patient_id": "pat-sj-001", "detail_level": "standard"},
        )
        assert not res.is_error
        bundle = json.loads(res.content[0].text)
        assert bundle["resourceType"] == "Bundle"
        assert len(bundle.get("entry", [])) >= 1

        obs_id = bundle["entry"][0]["resource"]["id"]
        res_obs = await server.call_tool("medplum_get_observation", {"observation_id": obs_id})
        assert not res_obs.is_error
        obs = json.loads(res_obs.content[0].text)
        assert obs["id"] == obs_id

    @pytest.mark.anyio
    async def test_condition_tools(self, audit_manager: AuditLogManager) -> None:
        """Test medplum_list_conditions and medplum_get_condition."""
        server = create_server(demo_mode=True, allow_writes=False, audit_manager=audit_manager)

        res = await server.call_tool(
            "medplum_list_conditions",
            {"patient_id": "pat-sj-001"},
        )
        assert not res.is_error
        bundle = json.loads(res.content[0].text)
        assert bundle["resourceType"] == "Bundle"
        assert len(bundle.get("entry", [])) >= 1

        cond_id = bundle["entry"][0]["resource"]["id"]
        res_cond = await server.call_tool("medplum_get_condition", {"condition_id": cond_id})
        assert not res_cond.is_error
        cond = json.loads(res_cond.content[0].text)
        assert cond["id"] == cond_id

    @pytest.mark.anyio
    async def test_medication_tools(self, audit_manager: AuditLogManager) -> None:
        """Test medplum_list_medication_requests and medplum_get_medication_request."""
        server = create_server(demo_mode=True, allow_writes=False, audit_manager=audit_manager)

        res = await server.call_tool(
            "medplum_list_medication_requests",
            {"patient_id": "pat-sj-001"},
        )
        assert not res.is_error
        bundle = json.loads(res.content[0].text)
        assert bundle["resourceType"] == "Bundle"
        assert len(bundle.get("entry", [])) >= 1

        med_id = bundle["entry"][0]["resource"]["id"]
        res_med = await server.call_tool("medplum_get_medication_request", {"request_id": med_id})
        assert not res_med.is_error
        med = json.loads(res_med.content[0].text)
        assert med["id"] == med_id

    @pytest.mark.anyio
    async def test_allergy_diagnostic_encounter_careplan_tools(
        self, audit_manager: AuditLogManager
    ) -> None:
        """Test allergy, diagnostic report, encounter, and care plan query tools."""
        server = create_server(demo_mode=True, allow_writes=False, audit_manager=audit_manager)

        # Allergy
        res_all = await server.call_tool(
            "medplum_list_allergy_intolerances", {"patient_id": "pat-sj-001"}
        )
        assert not res_all.is_error
        all_bundle = json.loads(res_all.content[0].text)
        all_id = all_bundle["entry"][0]["resource"]["id"]
        res_get_all = await server.call_tool(
            "medplum_get_allergy_intolerance", {"allergy_id": all_id}
        )
        assert not res_get_all.is_error

        # Diagnostic Report
        res_diag = await server.call_tool(
            "medplum_list_diagnostic_reports", {"patient_id": "pat-sj-001"}
        )
        assert not res_diag.is_error
        diag_bundle = json.loads(res_diag.content[0].text)
        diag_id = diag_bundle["entry"][0]["resource"]["id"]
        res_get_diag = await server.call_tool(
            "medplum_get_diagnostic_report", {"report_id": diag_id}
        )
        assert not res_get_diag.is_error

        # Encounter
        res_enc = await server.call_tool("medplum_list_encounters", {"patient_id": "pat-sj-001"})
        assert not res_enc.is_error

        # CarePlan
        res_cp = await server.call_tool("medplum_list_care_plans", {"patient_id": "pat-sj-001"})
        assert not res_cp.is_error


# ===========================================================================
# 2. Draft Mutations & Non-Bypassable Safety Gates
# ===========================================================================


class TestDraftMutationsAndSafetyGates:
    """Test create draft operations, write permission enforcement, and status invariants."""

    @pytest.mark.anyio
    async def test_create_observation_draft_success_when_writes_allowed(
        self, audit_manager: AuditLogManager
    ) -> None:
        """Verify draft observation creation succeeds when allow_writes=True."""
        server = create_server(demo_mode=True, allow_writes=True, audit_manager=audit_manager)

        res = await server.call_tool(
            "medplum_create_observation_draft",
            {
                "patient_id": "pat-sj-001",
                "code": "26499-4",
                "value_quantity": 1500.0,
                "unit": "/uL",
                "display": "Absolute Neutrophil Count",
            },
        )
        assert not res.is_error
        obs = json.loads(res.content[0].text)
        assert obs["id"] is not None
        assert obs["status"] == "registered"

        # Audit flight recorder logged ALLOWED
        entries = audit_manager.get_entries()
        assert entries[-1].tool_name == "medplum_create_observation_draft"
        assert entries[-1].action_status == ActionStatus.ALLOWED

    @pytest.mark.anyio
    async def test_create_medication_draft_success_when_writes_allowed(
        self, audit_manager: AuditLogManager
    ) -> None:
        """Verify draft medication request creation succeeds when allow_writes=True."""
        server = create_server(demo_mode=True, allow_writes=True, audit_manager=audit_manager)

        res = await server.call_tool(
            "medplum_create_medication_draft",
            {
                "patient_id": "pat-sj-001",
                "medication_code": "115113",
                "dosage": "1.5 mg/m2",
                "instructions": "Administer IV over 15 minutes once weekly",
                "medication_display": "Vincristine sulfate",
            },
        )
        assert not res.is_error
        med = json.loads(res.content[0].text)
        assert med["id"] is not None
        assert med["status"] == "draft"

        # Audit flight recorder logged ALLOWED
        entries = audit_manager.get_entries()
        assert entries[-1].tool_name == "medplum_create_medication_draft"
        assert entries[-1].action_status == ActionStatus.ALLOWED

    @pytest.mark.anyio
    async def test_mutation_rejected_and_logged_blocked_when_allow_writes_false(
        self, audit_manager: AuditLogManager
    ) -> None:
        """Verify mutation when allow_writes=False raises PermissionError and logs BLOCKED."""
        server = create_server(demo_mode=True, allow_writes=False, audit_manager=audit_manager)

        # Observation draft write blocked
        with pytest.raises(PermissionError):
            await server.call_tool(
                "medplum_create_observation_draft",
                {
                    "patient_id": "pat-sj-001",
                    "code": "26499-4",
                    "value_quantity": 1500.0,
                    "unit": "/uL",
                },
            )

        entries = audit_manager.get_entries()
        assert entries[-1].tool_name == "medplum_create_observation_draft"
        assert entries[-1].action_status == ActionStatus.BLOCKED

        # Medication draft write blocked
        with pytest.raises(PermissionError):
            await server.call_tool(
                "medplum_create_medication_draft",
                {
                    "patient_id": "pat-sj-001",
                    "medication_code": "115113",
                    "dosage": "1 mg",
                    "instructions": "Daily",
                },
            )

        entries = audit_manager.get_entries()
        assert entries[-1].tool_name == "medplum_create_medication_draft"
        assert entries[-1].action_status == ActionStatus.BLOCKED

    @pytest.mark.anyio
    async def test_mutation_attempting_forbidden_status_raises_safety_violation_and_logs_blocked(
        self, audit_manager: AuditLogManager
    ) -> None:
        """Verify mutation attempting forbidden status raises SafetyInvariantViolation and logs."""
        server = create_server(demo_mode=True, allow_writes=True, audit_manager=audit_manager)

        # Attempt forbidden status on observation
        with pytest.raises(SafetyInvariantViolation):
            await server.call_tool(
                "medplum_create_observation_draft",
                {
                    "patient_id": "pat-sj-001",
                    "code": "26499-4",
                    "value_quantity": 1500.0,
                    "unit": "/uL",
                    "status": "final",
                },
            )

        entries = audit_manager.get_entries()
        assert entries[-1].tool_name == "medplum_create_observation_draft"
        assert entries[-1].action_status == ActionStatus.BLOCKED

        # Attempt forbidden status 'active' on medication
        with pytest.raises(SafetyInvariantViolation):
            await server.call_tool(
                "medplum_create_medication_draft",
                {
                    "patient_id": "pat-sj-001",
                    "medication_code": "115113",
                    "dosage": "1.5 mg",
                    "instructions": "Stat",
                    "status": "active",
                },
            )

        entries = audit_manager.get_entries()
        assert entries[-1].tool_name == "medplum_create_medication_draft"
        assert entries[-1].action_status == ActionStatus.BLOCKED


# ===========================================================================
# 3. Native FHIR Resources (@server.resource)
# ===========================================================================


class TestNativeFHIRResources:
    """Test the 4 native FHIR resources exposed by FastMCP."""

    @pytest.mark.anyio
    async def test_patient_clinical_summary_resource(self, audit_manager: AuditLogManager) -> None:
        """Verify fhir://patients/{patient_id}/clinical-summary resource."""
        server = create_server(demo_mode=True, allow_writes=False, audit_manager=audit_manager)

        contents = await server.read_resource("fhir://patients/pat-sj-001/clinical-summary")
        assert isinstance(contents, list)
        assert len(contents) > 0
        raw_text = contents[0].content
        assert isinstance(raw_text, str)
        data = json.loads(raw_text)

        assert data["patient_id"] == "pat-sj-001"
        assert "patient" in data
        assert "conditions" in data
        assert "medications" in data
        assert "allergies" in data
        assert "observations" in data

    @pytest.mark.anyio
    async def test_benchmarks_resource(self, audit_manager: AuditLogManager) -> None:
        """Verify fhir://benchmarks returns token distillation metrics."""
        server = create_server(demo_mode=True, allow_writes=False, audit_manager=audit_manager)

        contents = await server.read_resource("fhir://benchmarks")
        assert isinstance(contents, list)
        assert len(contents) > 0
        raw_text = contents[0].content
        assert isinstance(raw_text, str)
        data = json.loads(raw_text)

        assert "overall_reduction_pct" in data
        assert data["overall_reduction_pct"] >= 85.0
        assert "aggregate_standard_reduction_pct" in data

    @pytest.mark.anyio
    async def test_audit_latest_resource(self, audit_manager: AuditLogManager) -> None:
        """Verify fhir://audit/latest returns recent audit records."""
        server = create_server(demo_mode=True, allow_writes=False, audit_manager=audit_manager)

        # Trigger a tool call so an audit entry exists
        await server.call_tool("medplum_get_patient", {"patient_id": "pat-sj-001"})

        contents = await server.read_resource("fhir://audit/latest")
        assert isinstance(contents, list)
        assert len(contents) > 0
        raw_text = contents[0].content
        assert isinstance(raw_text, str)
        entries = json.loads(raw_text)
        assert isinstance(entries, list)
        assert len(entries) >= 1
        assert entries[-1]["tool_name"] == "medplum_get_patient"

    @pytest.mark.anyio
    async def test_verification_status_resource(self, audit_manager: AuditLogManager) -> None:
        """Verify fhir://verification/status returns deterministic safety status."""
        server = create_server(demo_mode=True, allow_writes=False, audit_manager=audit_manager)

        contents = await server.read_resource("fhir://verification/status")
        assert isinstance(contents, list)
        assert len(contents) > 0
        raw_text = contents[0].content
        assert isinstance(raw_text, str)
        data = json.loads(raw_text)

        assert data["overall_status"] == "ENFORCED"
        assert "entities" in data
        for _entity_name, entity_proof in data["entities"].items():
            assert entity_proof["enforced"] is True
            assert "blocked_statuses" in entity_proof


# ===========================================================================
# 4. Native Prompts (@server.prompt)
# ===========================================================================


class TestNativePrompts:
    """Test the 2 native clinical prompts exposed by FastMCP."""

    @pytest.mark.anyio
    async def test_clinical_encounter_triage_prompt(self, audit_manager: AuditLogManager) -> None:
        """Verify clinical_encounter_triage prompt contains vitals and witness requirement."""
        server = create_server(demo_mode=True, allow_writes=False, audit_manager=audit_manager)

        prompt_res = await server.get_prompt(
            "clinical_encounter_triage",
            {"patient_id": "pat-sj-001", "encounter_id": "enc-sj-001"},
        )
        assert isinstance(prompt_res, GetPromptResult)
        assert len(prompt_res.messages) > 0
        msg_content = prompt_res.messages[0].content
        assert isinstance(msg_content, TextContent)
        text = msg_content.text

        assert "pat-sj-001" in text
        assert "enc-sj-001" in text
        assert "vitals" in text.lower()
        assert "anomal" in text.lower()
        assert "physician" in text.lower()
        assert "witness" in text.lower()

    @pytest.mark.anyio
    async def test_drug_interaction_audit_prompt(self, audit_manager: AuditLogManager) -> None:
        """Verify drug_interaction_audit prompt cross-references allergies and medications."""
        server = create_server(demo_mode=True, allow_writes=False, audit_manager=audit_manager)

        prompt_res = await server.get_prompt(
            "drug_interaction_audit",
            {"patient_id": "pat-sj-001", "proposed_medication": "Methotrexate"},
        )
        assert isinstance(prompt_res, GetPromptResult)
        assert len(prompt_res.messages) > 0
        msg_content = prompt_res.messages[0].content
        assert isinstance(msg_content, TextContent)
        text = msg_content.text

        assert "pat-sj-001" in text
        assert "Methotrexate" in text
        assert "allerg" in text.lower()
        assert "medication" in text.lower()


# ===========================================================================
# 5. CLI Entrypoint & Argument Parsing
# ===========================================================================


class TestCLIEntrypoint:
    """Test CLI argument parsing and subcommand execution."""

    def test_root_parser_flags_defaults(self) -> None:
        """Verify default root CLI flags."""
        parser = build_parser()
        args = parser.parse_args([])
        assert args.command is None
        assert args.transport == "stdio"
        assert args.host == "0.0.0.0"
        assert args.port == 8000
        assert args.demo is False
        assert args.allow_writes is False
        assert args.base_url == "https://api.medplum.com"
        assert args.audit_log_path is None
        assert args.audit_key is None

    def test_root_parser_custom_flags(self) -> None:
        """Verify custom root CLI flags."""
        parser = build_parser()
        args = parser.parse_args(
            [
                "--transport",
                "sse",
                "--host",
                "127.0.0.1",
                "--port",
                "9090",
                "--demo",
                "--allow-writes",
                "--base-url",
                "https://my-medplum.org",
                "--audit-log-path",
                "/var/log/audit.jsonl",
                "--audit-key",
                "my-secret-key",
            ]
        )
        assert args.transport == "sse"
        assert args.host == "127.0.0.1"
        assert args.port == 9090
        assert args.demo is True
        assert args.allow_writes is True
        assert args.base_url == "https://my-medplum.org"
        assert args.audit_log_path == "/var/log/audit.jsonl"
        assert args.audit_key == "my-secret-key"

    def test_mock_server_subcommand_parsing(self) -> None:
        """Verify mock-server subcommand argument parsing."""
        parser = build_parser()
        args = parser.parse_args(["mock-server", "--host", "0.0.0.0", "--port", "8088"])
        assert args.command == "mock-server"
        assert args.host == "0.0.0.0"
        assert args.port == 8088

    def test_audit_verify_subcommand_parsing(self) -> None:
        """Verify audit verify subcommand parsing."""
        parser = build_parser()
        args = parser.parse_args(["audit", "verify", "--log-path", "audit.jsonl", "--key", "mykey"])
        assert args.command == "audit"
        assert args.audit_action == "verify"
        assert args.log_path == "audit.jsonl"
        assert args.key == "mykey"

    def test_audit_inspect_subcommand_parsing(self) -> None:
        """Verify audit inspect subcommand parsing."""
        parser = build_parser()
        args = parser.parse_args(
            [
                "audit",
                "inspect",
                "--log-path",
                "audit.jsonl",
                "--limit",
                "25",
                "--blocked-only",
            ]
        )
        assert args.command == "audit"
        assert args.audit_action == "inspect"
        assert args.log_path == "audit.jsonl"
        assert args.limit == 25
        assert args.blocked_only is True

    def test_create_parser_alias(self) -> None:
        """Verify create_parser is an alias to build_parser."""
        parser = create_parser()
        assert parser is not None
        assert parser.prog == "medplum-mcp"

    def test_config_subcommand_parsing(self) -> None:
        """Verify config subcommand flags parsing."""
        parser = build_parser()
        args = parser.parse_args(
            [
                "config",
                "--client",
                "cursor",
                "--demo",
                "--transport",
                "sse",
                "--url",
                "http://example.com/sse",
                "--install",
                "--output-dir",
                "/tmp/configs",
                "--config-path",
                "/tmp/configs/mcp.json",
            ]
        )
        assert args.command == "config"
        assert args.client == "cursor"
        assert args.demo is True
        assert args.transport == "sse"
        assert args.url == "http://example.com/sse"
        assert args.install is True
        assert args.output_dir == "/tmp/configs"
        assert args.config_path == "/tmp/configs/mcp.json"

    def test_verify_subcommand_parsing(self) -> None:
        """Verify verify subcommand flags parsing."""
        parser = build_parser()
        args = parser.parse_args(
            [
                "verify",
                "--quick",
                "--audit-log",
                "/tmp/audit.jsonl",
                "--audit-key",
                "secret-key",
                "--output",
                "markdown",
                "--strict",
            ]
        )
        assert args.command == "verify"
        assert args.quick is True
        assert args.audit_log == "/tmp/audit.jsonl"
        assert args.audit_key == "secret-key"
        assert args.output == "markdown"
        assert args.strict is True

    def test_main_mock_server_execution(self) -> None:
        """Verify main() executes mock-server when subcommand given."""
        with patch("medplum_mcp.server.MedplumMockServer") as mock_server_cls:
            mock_inst = MagicMock()
            mock_server_cls.return_value = mock_inst
            mock_inst.serve_forever.side_effect = KeyboardInterrupt

            exit_code = main(["mock-server", "--port", "8099"])
            assert exit_code == 0
            mock_server_cls.assert_called_once_with(host="127.0.0.1", port=8099)
            mock_inst.serve_forever.assert_called_once()

    def test_main_audit_verify_execution(self, audit_log_path: Path) -> None:
        """Verify main() executes audit verify correctly."""
        manager = AuditLogManager(audit_log_path, "secret-key")
        manager.log_event("tool_1", ActionStatus.ALLOWED)

        exit_code = main(
            [
                "audit",
                "verify",
                "--log-path",
                str(audit_log_path),
                "--key",
                "secret-key",
            ]
        )
        assert exit_code == 0

    def test_main_audit_inspect_execution(
        self, audit_log_path: Path, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """Verify main() executes audit inspect correctly."""
        manager = AuditLogManager(audit_log_path, "secret-key")
        manager.log_event("tool_1", ActionStatus.ALLOWED)
        manager.log_event("tool_2", ActionStatus.BLOCKED)

        exit_code = main(
            [
                "audit",
                "inspect",
                "--log-path",
                str(audit_log_path),
                "--blocked-only",
            ]
        )
        assert exit_code == 0
        captured = capsys.readouterr().out
        assert "tool_2" in captured
        assert "tool_1" not in captured

    def test_main_server_run_execution(self) -> None:
        """Verify main() runs server with configured transport."""
        with patch("medplum_mcp.server.create_server") as mock_create:
            mock_srv = MagicMock()
            mock_create.return_value = mock_srv

            exit_code = main(["--demo", "--transport", "stdio"])
            assert exit_code == 0
            mock_create.assert_called_once()
            mock_srv.run.assert_called_once_with(transport="stdio")
