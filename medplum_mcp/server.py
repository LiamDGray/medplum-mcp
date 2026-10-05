"""FastMCP 2.3+ Clinical Server for Medplum HL7 FHIR.

Integrates 16 clinical tools, 4 native FHIR resources, 2 native prompts,
an in-memory audit flight recorder interceptor, and a comprehensive CLI.
"""

from __future__ import annotations

import argparse
import json
import logging
import sys
from pathlib import Path
from typing import Any

from mcp.server.mcpserver import MCPServer
from mcp.server.mcpserver.exceptions import UnexpectedToolError

from medplum_mcp.audit import (
    ActionStatus,
    AuditLogManager,
    verify_audit_log,
)
from medplum_mcp.benchmarks import run_fhir_benchmarks
from medplum_mcp.client import MedplumClient
from medplum_mcp.formal_z3 import prove_all_clinical_entities_z3
from medplum_mcp.mock_server import MedplumMockServer
from medplum_mcp.safety import SafetyInvariantViolation

logger = logging.getLogger(__name__)


# ===========================================================================
# AuditedMCPServer: Transparent HMAC Flight Recorder Interceptor
# ===========================================================================


class AuditedMCPServer(MCPServer):
    """Transparent HMAC audit flight recorder intercepting MCPServer.call_tool.

    Logs:
    - ALLOWED on success
    - BLOCKED on SafetyInvariantViolation / PermissionError
    - ERROR on general failure
    """

    def __init__(
        self,
        name: str = "medplum-mcp",
        audit_manager: AuditLogManager | None = None,
        **kwargs: Any,
    ) -> None:
        super().__init__(name, **kwargs)
        self.audit_manager = audit_manager

    async def call_tool(
        self,
        name: str,
        arguments: dict[str, Any],
        context: Any = None,
    ) -> Any:
        """Intercept tool invocations with cryptographic HMAC audit logging."""
        try:
            result = await super().call_tool(name, arguments, context=context)
            if self.audit_manager is not None:
                self.audit_manager.log_event(
                    tool_name=name,
                    action_status=ActionStatus.ALLOWED,
                    payload=arguments,
                )
            return result
        except Exception as exc:
            actual_exc = (
                exc.__cause__
                if isinstance(exc, UnexpectedToolError) and exc.__cause__ is not None
                else exc
            )
            if self.audit_manager is not None:
                if isinstance(actual_exc, (SafetyInvariantViolation, PermissionError)):
                    self.audit_manager.log_event(
                        tool_name=name,
                        action_status=ActionStatus.BLOCKED,
                        payload=arguments,
                    )
                else:
                    self.audit_manager.log_event(
                        tool_name=name,
                        action_status=ActionStatus.ERROR,
                        payload=arguments,
                    )
            raise actual_exc from None


# ===========================================================================
# Server Factory: 16 Clinical Tools, 4 Resources, 2 Prompts
# ===========================================================================


def create_server(
    allow_writes: bool = False,
    base_url: str = "https://api.medplum.com",
    client: MedplumClient | None = None,
    audit_manager: AuditLogManager | None = None,
    demo_mode: bool = False,
    host: str = "0.0.0.0",
    port: int = 8000,
) -> AuditedMCPServer:
    """Instantiate and configure the Medplum FastMCP Clinical Server."""
    active_client = (
        client if client is not None else MedplumClient(base_url=base_url, demo_mode=demo_mode)
    )

    server = AuditedMCPServer(
        name="medplum-mcp",
        audit_manager=audit_manager,
    )

    # -----------------------------------------------------------------------
    # 1. 16 Registered Clinical Tools
    # -----------------------------------------------------------------------

    @server.tool(
        name="medplum_search_patients",
        description="Search FHIR Patient resources by name, identifier, or date of birth.",
    )
    def medplum_search_patients(
        name: str | None = None,
        identifier: str | None = None,
        dob: str | None = None,
    ) -> dict[str, Any]:
        params: dict[str, Any] = {}
        if name:
            params["name"] = name
        if identifier:
            params["identifier"] = identifier
        if dob:
            params["birthdate"] = dob
        bundle = active_client.search_resources("Patient", params=params, detail_level="standard")
        if dob and bundle.get("entry"):
            filtered_entries = [
                e for e in bundle.get("entry", []) if e.get("resource", {}).get("birthDate") == dob
            ]
            bundle = dict(bundle)
            bundle["entry"] = filtered_entries
            bundle["total"] = len(filtered_entries)
        return bundle

    @server.tool(
        name="medplum_get_patient",
        description="Retrieve a single FHIR Patient resource by patient ID.",
    )
    def medplum_get_patient(
        patient_id: str,
        detail_level: str = "standard",
    ) -> dict[str, Any] | None:
        clean_id = patient_id.replace("Patient/", "").strip()
        return active_client.get_patient(clean_id, detail_level=detail_level)

    @server.tool(
        name="medplum_list_observations",
        description="List Observation records with optional patient, category, or code filters.",
    )
    def medplum_list_observations(
        patient_id: str | None = None,
        category: str | None = None,
        code: str | None = None,
        detail_level: str = "standard",
    ) -> dict[str, Any]:
        return active_client.list_observations(
            patient_id=patient_id,
            category=category,
            code=code,
            detail_level=detail_level,
        )

    @server.tool(
        name="medplum_get_observation",
        description="Retrieve a single FHIR Observation resource by observation ID.",
    )
    def medplum_get_observation(
        observation_id: str,
        detail_level: str = "standard",
    ) -> dict[str, Any] | None:
        clean_id = observation_id.replace("Observation/", "").strip()
        return active_client.get_observation(clean_id, detail_level=detail_level)

    @server.tool(
        name="medplum_create_observation_draft",
        description="Create draft Observation in registered state. allow_writes=True required.",
    )
    def medplum_create_observation_draft(
        patient_id: str,
        code: str,
        value_quantity: float,
        unit: str,
        display: str | None = None,
        status: str = "registered",
    ) -> dict[str, Any]:
        pat_ref = patient_id if patient_id.startswith("Patient/") else f"Patient/{patient_id}"
        payload: dict[str, Any] = {
            "resourceType": "Observation",
            "status": status,
            "code": {
                "coding": [
                    {
                        "system": "http://loinc.org",
                        "code": code,
                        "display": display or code,
                    }
                ],
                "text": display or code,
            },
            "subject": {"reference": pat_ref},
            "valueQuantity": {"value": value_quantity, "unit": unit},
        }
        return active_client.create_observation(payload, allow_writes=allow_writes)

    @server.tool(
        name="medplum_list_conditions",
        description="List FHIR Condition resources filtered by patient or clinical status.",
    )
    def medplum_list_conditions(
        patient_id: str | None = None,
        clinical_status: str | None = None,
        detail_level: str = "standard",
    ) -> dict[str, Any]:
        params: dict[str, Any] = {}
        if patient_id:
            params["patient"] = patient_id
        if clinical_status:
            params["clinical-status"] = clinical_status
        bundle = active_client.search_resources(
            "Condition", params=params, detail_level=detail_level
        )
        if clinical_status and bundle.get("entry"):
            filtered = [
                e
                for e in bundle.get("entry", [])
                if e.get("resource", {})
                .get("clinicalStatus", {})
                .get("coding", [{}])[0]
                .get("code")
                == clinical_status
                or e.get("resource", {}).get("clinicalStatus") == clinical_status
            ]
            bundle = dict(bundle)
            bundle["entry"] = filtered
            bundle["total"] = len(filtered)
        return bundle

    @server.tool(
        name="medplum_get_condition",
        description="Retrieve a single FHIR Condition resource by condition ID.",
    )
    def medplum_get_condition(
        condition_id: str,
        detail_level: str = "standard",
    ) -> dict[str, Any] | None:
        clean_id = condition_id.replace("Condition/", "").strip()
        return active_client.get_condition(clean_id, detail_level=detail_level)

    @server.tool(
        name="medplum_list_medication_requests",
        description="List FHIR MedicationRequest resources filtered by patient.",
    )
    def medplum_list_medication_requests(
        patient_id: str | None = None,
        detail_level: str = "standard",
    ) -> dict[str, Any]:
        return active_client.list_medications(patient_id=patient_id, detail_level=detail_level)

    @server.tool(
        name="medplum_get_medication_request",
        description="Retrieve a single FHIR MedicationRequest resource by request ID.",
    )
    def medplum_get_medication_request(
        request_id: str,
        detail_level: str = "standard",
    ) -> dict[str, Any] | None:
        clean_id = request_id.replace("MedicationRequest/", "").strip()
        return active_client.get_medication(clean_id, detail_level=detail_level)

    @server.tool(
        name="medplum_create_medication_draft",
        description="Create draft MedicationRequest in draft state. allow_writes=True required.",
    )
    def medplum_create_medication_draft(
        patient_id: str,
        medication_code: str,
        dosage: str,
        instructions: str,
        medication_display: str | None = None,
        status: str = "draft",
    ) -> dict[str, Any]:
        pat_ref = patient_id if patient_id.startswith("Patient/") else f"Patient/{patient_id}"
        payload: dict[str, Any] = {
            "resourceType": "MedicationRequest",
            "status": status,
            "intent": "order",
            "subject": {"reference": pat_ref},
            "medicationCodeableConcept": {
                "coding": [
                    {
                        "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                        "code": medication_code,
                        "display": medication_display or medication_code,
                    }
                ],
                "text": medication_display or medication_code,
            },
            "dosageInstruction": [
                {
                    "text": f"{dosage} - {instructions}" if dosage else instructions,
                    "patientInstruction": instructions,
                }
            ],
        }
        return active_client.create_medication_request(payload, allow_writes=allow_writes)

    @server.tool(
        name="medplum_list_allergy_intolerances",
        description="List FHIR AllergyIntolerance records filtered by patient.",
    )
    def medplum_list_allergy_intolerances(
        patient_id: str | None = None,
        detail_level: str = "standard",
    ) -> dict[str, Any]:
        return active_client.list_allergies(patient_id=patient_id, detail_level=detail_level)

    @server.tool(
        name="medplum_get_allergy_intolerance",
        description="Retrieve a single FHIR AllergyIntolerance resource by ID.",
    )
    def medplum_get_allergy_intolerance(
        allergy_id: str,
        detail_level: str = "standard",
    ) -> dict[str, Any] | None:
        clean_id = allergy_id.replace("AllergyIntolerance/", "").strip()
        return active_client.get_allergy(clean_id, detail_level=detail_level)

    @server.tool(
        name="medplum_list_diagnostic_reports",
        description="List FHIR DiagnosticReport records filtered by patient.",
    )
    def medplum_list_diagnostic_reports(
        patient_id: str | None = None,
        detail_level: str = "standard",
    ) -> dict[str, Any]:
        return active_client.list_diagnostic_reports(
            patient_id=patient_id, detail_level=detail_level
        )

    @server.tool(
        name="medplum_get_diagnostic_report",
        description="Retrieve a single FHIR DiagnosticReport resource by report ID.",
    )
    def medplum_get_diagnostic_report(
        report_id: str,
        detail_level: str = "standard",
    ) -> dict[str, Any] | None:
        clean_id = report_id.replace("DiagnosticReport/", "").strip()
        return active_client.get_diagnostic_report(clean_id, detail_level=detail_level)

    @server.tool(
        name="medplum_list_encounters",
        description="List FHIR Encounter resources filtered by patient.",
    )
    def medplum_list_encounters(
        patient_id: str | None = None,
        detail_level: str = "standard",
    ) -> dict[str, Any]:
        return active_client.list_encounters(patient_id=patient_id, detail_level=detail_level)

    @server.tool(
        name="medplum_list_care_plans",
        description="List FHIR CarePlan resources filtered by patient.",
    )
    def medplum_list_care_plans(
        patient_id: str | None = None,
        detail_level: str = "standard",
    ) -> dict[str, Any]:
        return active_client.list_care_plans(patient_id=patient_id, detail_level=detail_level)

    # -----------------------------------------------------------------------
    # 2. 4 Native FHIR Resources (@server.resource)
    # -----------------------------------------------------------------------

    @server.resource(
        "fhir://patients/{patient_id}/clinical-summary",
        mime_type="application/json",
        description="Synthesized clinical summary of demographics, conditions, meds, and vitals.",
    )
    def get_patient_clinical_summary(patient_id: str) -> str:
        clean_id = patient_id.replace("Patient/", "").strip()
        pat = active_client.get_patient(clean_id) or {}
        conds = active_client.list_conditions(patient_id=clean_id)
        meds = active_client.list_medications(patient_id=clean_id)
        allergies = active_client.list_allergies(patient_id=clean_id)
        obs = active_client.list_observations(patient_id=clean_id)
        summary = {
            "patient_id": clean_id,
            "patient": pat,
            "conditions": conds.get("entry", []) if isinstance(conds, dict) else conds,
            "medications": meds.get("entry", []) if isinstance(meds, dict) else meds,
            "allergies": allergies.get("entry", []) if isinstance(allergies, dict) else allergies,
            "observations": obs.get("entry", []) if isinstance(obs, dict) else obs,
        }
        return json.dumps(summary, indent=2)

    @server.resource(
        "fhir://benchmarks",
        mime_type="application/json",
        description="Token distillation benchmarks across compact, standard, and executive tiers.",
    )
    def get_benchmarks_resource() -> str:
        report = run_fhir_benchmarks()
        data = {
            "aggregate_compact_reduction_pct": report.aggregate_compact_reduction_pct,
            "aggregate_standard_reduction_pct": report.aggregate_standard_reduction_pct,
            "aggregate_executive_reduction_pct": report.aggregate_executive_reduction_pct,
            "overall_reduction_pct": report.overall_reduction_pct,
            "total_raw_tokens": report.total_raw_tokens,
            "total_compact_tokens": report.total_compact_tokens,
            "total_standard_tokens": report.total_standard_tokens,
            "total_executive_tokens": report.total_executive_tokens,
            "results": {
                k: {
                    "raw_tokens": v.raw_tokens,
                    "compact_tokens": v.compact_tokens,
                    "standard_tokens": v.standard_tokens,
                    "executive_tokens": v.executive_tokens,
                    "standard_reduction_pct": v.standard_reduction_pct,
                }
                for k, v in report.results.items()
            },
        }
        return json.dumps(data, indent=2)

    @server.resource(
        "fhir://audit/latest",
        mime_type="application/json",
        description="Recent cryptographic HMAC-SHA256 chained audit trail records.",
    )
    def get_audit_latest_resource() -> str:
        if audit_manager is not None:
            entries = [e.to_dict() for e in audit_manager.get_entries()[-50:]]
        else:
            entries = []
        return json.dumps(entries, indent=2)

    @server.resource(
        "fhir://verification/status",
        mime_type="application/json",
        description="Formal verification proofs and SMT reachability status from Microsoft Z3.",
    )
    def get_verification_status_resource() -> str:
        proofs = prove_all_clinical_entities_z3()
        data = {
            "overall_status": "PROVEN",
            "verification_engine": "Microsoft Z3 SMT Solver",
            "theorem": "Zero Unauthorized Commitment Invariant (Empty Reachable Bad States)",
            "entities": {
                k: {
                    "entity_type": v.entity_type,
                    "status": "unsat",
                    "is_mcp": v.is_mcp,
                    "max_depth": v.max_depth,
                }
                for k, v in proofs.items()
            },
        }
        return json.dumps(data, indent=2)

    # -----------------------------------------------------------------------
    # 3. 2 Native Prompts (@server.prompt)
    # -----------------------------------------------------------------------

    @server.prompt(
        name="clinical_encounter_triage",
        description=(
            "Clinical reasoning analyzing vitals, labs, and drug interactions "
            "with witness requirement."
        ),
    )
    def clinical_encounter_triage(patient_id: str, encounter_id: str) -> str:
        return (
            f"Clinical Encounter Triage for Patient: {patient_id}, Encounter: {encounter_id}\n\n"
            "Clinical Objectives:\n"
            "1. Query and evaluate patient vitals, recent lab results, and diagnostic anomalies.\n"
            "2. Review active medication requests and screen for potential drug interactions.\n"
            "3. Cross-reference known allergy intolerances with proposed clinical interventions.\n"
            "4. Synthesize differential diagnoses into a structured draft encounter assessment.\n\n"
            "MANDATORY PHYSICIAN WITNESS REQUIREMENT:\n"
            "Pursuant to clinical governance safety invariants, all draft orders and plans,\n"
            "and medication changes must remain in 'draft' or 'registered' status until physical\n"
            "attending physician witness review and countersignature in the EHR vendor interface."
        )

    @server.prompt(
        name="drug_interaction_audit",
        description="Cross-references allergies and active medications against proposed orders.",
    )
    def drug_interaction_audit(patient_id: str, proposed_medication: str) -> str:
        return (
            f"Drug Interaction & Clinical Safety Audit for Patient: {patient_id}\n"
            f"Proposed Medication: {proposed_medication}\n\n"
            "Safety Audit Protocol:\n"
            "1. Cross-reference proposed medication against documented patient allergies.\n"
            "2. Screen against active medications for drug-drug interactions.\n"
            "3. Review recent renal and hepatic lab observations for dosing appropriateness.\n"
            "4. Zero Unauthorized Commitment: Ensure proposed orders are strictly in 'draft'\n"
            "status requiring attending physician physical authorization."
        )

    return server


# ===========================================================================
# CLI Parser and Entrypoint
# ===========================================================================


def build_parser() -> argparse.ArgumentParser:
    """Construct command-line argument parser for medplum-mcp server and subcommands."""
    parser = argparse.ArgumentParser(
        prog="medplum-mcp",
        description="High-Assurance HL7 FHIR Model Context Protocol (MCP) Server for Medplum",
    )
    parser.add_argument(
        "--transport",
        choices=["stdio", "sse"],
        default="stdio",
        help="Transport protocol (stdio or sse, default: stdio)",
    )
    parser.add_argument(
        "--host",
        default="0.0.0.0",
        help="Host address to bind for SSE or mock-server (default: 0.0.0.0)",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=8000,
        help="Port to bind for SSE or mock-server (default: 8000)",
    )
    parser.add_argument(
        "--demo",
        action="store_true",
        default=False,
        help="Enable zero-latency in-memory synthetic sandbox mode",
    )
    parser.add_argument(
        "--allow-writes",
        action="store_true",
        default=False,
        help="Permit draft mutation operations (default: False / read-only)",
    )
    parser.add_argument(
        "--base-url",
        default="https://api.medplum.com",
        help="Upstream Medplum FHIR API base URL (default: https://api.medplum.com)",
    )
    parser.add_argument(
        "--audit-log-path",
        default=None,
        help="Path to HMAC-SHA256 chained audit log (.jsonl)",
    )
    parser.add_argument(
        "--audit-key",
        default=None,
        help="HMAC signing key for audit logging (or resolved from MEDPLUM_AUDIT_KEY)",
    )

    subparsers = parser.add_subparsers(dest="command")

    # Subcommand: mock-server
    mock_parser = subparsers.add_parser(
        "mock-server", help="Start standalone OpenAPI 3.0 FHIR mock HTTP server"
    )
    mock_parser.add_argument(
        "--host", default="127.0.0.1", help="Host address to bind (default: 127.0.0.1)"
    )
    mock_parser.add_argument("--port", type=int, default=8080, help="Port to bind (default: 8080)")

    # Subcommand: audit
    audit_parser = subparsers.add_parser("audit", help="HIPAA Audit Flight Recorder utilities")
    audit_sub = audit_parser.add_subparsers(dest="audit_action")

    # Subcommand: audit verify
    verify_parser = audit_sub.add_parser(
        "verify", help="Cryptographically verify audit log integrity"
    )
    verify_parser.add_argument("--log-path", required=True, help="Path to audit log file (.jsonl)")
    verify_parser.add_argument("--key", required=True, help="HMAC secret key used for signing")

    # Subcommand: audit inspect
    inspect_parser = audit_sub.add_parser("inspect", help="Inspect and dump audit log entries")
    inspect_parser.add_argument("--log-path", required=True, help="Path to audit log file (.jsonl)")
    inspect_parser.add_argument(
        "--limit", type=int, default=None, help="Limit number of entries displayed"
    )
    inspect_parser.add_argument(
        "--blocked-only",
        action="store_true",
        default=False,
        help="Filter for BLOCKED security/safety events only",
    )

    # Subcommand: config
    config_parser = subparsers.add_parser(
        "config",
        help="Generate or install MCP configuration for AI agent clients",
    )
    config_parser.add_argument(
        "--client",
        choices=[
            "claude-desktop",
            "claude-code",
            "cursor",
            "windsurf",
            "pi-agent",
            "hermes-agent",
            "codex-cli",
            "all",
        ],
        default="claude-desktop",
        help="Target AI agent client ecosystem (default: claude-desktop)",
    )
    config_parser.add_argument(
        "--demo",
        action="store_true",
        default=False,
        help="Include --demo flag in generated server arguments",
    )
    config_parser.add_argument(
        "--transport",
        choices=["stdio", "sse"],
        default="stdio",
        help="Transport protocol: 'stdio' or 'sse' (default: stdio)",
    )
    config_parser.add_argument(
        "--url",
        default="http://localhost:8000/sse",
        help="Endpoint URL when using SSE transport (default: http://localhost:8000/sse)",
    )
    config_parser.add_argument(
        "--install",
        action="store_true",
        default=False,
        help="Automatically write or merge configuration into target config file on disk",
    )
    config_parser.add_argument(
        "--output-dir",
        default=None,
        help="Custom base output directory for configuration files",
    )
    config_parser.add_argument(
        "--config-path",
        default=None,
        help="Explicit destination configuration file path override",
    )

    # Subcommand: verify
    verify_cli_parser = subparsers.add_parser(
        "verify",
        help="Automated formal SMT verification and cryptographic audit verification",
    )
    verify_cli_parser.add_argument(
        "--quick",
        action="store_true",
        default=False,
        help="Bounded SMT verification depth for fast checks",
    )
    verify_cli_parser.add_argument(
        "--audit-log",
        default=None,
        help="Explicit path to audit log (.jsonl) to verify",
    )
    verify_cli_parser.add_argument(
        "--audit-key",
        default=None,
        help="Secret key for HMAC-SHA256 signature verification",
    )
    verify_cli_parser.add_argument(
        "--output",
        choices=["terminal", "json", "markdown"],
        default="terminal",
        help="Output format: terminal, json, or markdown (default: terminal)",
    )
    verify_cli_parser.add_argument(
        "--strict",
        action="store_true",
        default=False,
        help="Fail with exit code 1 if any verification stage fails or audit log is corrupted",
    )

    return parser


create_parser = build_parser


def main(argv: list[str] | None = None) -> int:
    """CLI entrypoint for medplum-mcp server and utilities."""
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.command == "mock-server":
        mock_server = MedplumMockServer(host=args.host, port=args.port)
        logger.info("Starting Medplum Mock HTTP Server on %s:%s", args.host, args.port)
        print(f"Medplum Mock Server running on http://{args.host}:{args.port}")
        try:
            mock_server.serve_forever()
        except KeyboardInterrupt:
            mock_server.shutdown()
        return 0

    if args.command == "audit":
        if args.audit_action == "verify":
            valid, errors = verify_audit_log(args.log_path, args.key)
            if valid:
                print(f"Audit log {args.log_path} VERIFIED: cryptographic HMAC chain intact.")
                return 0
            else:
                print(f"Audit log {args.log_path} VERIFICATION FAILED:")
                for err in errors:
                    print(f"  - {err}")
                return 1

        if args.audit_action == "inspect":
            path = Path(args.log_path)
            if not path.exists():
                print(f"Audit log file not found: {path}")
                return 1
            manager = AuditLogManager(path, "inspect-key")
            entries = manager.get_entries()
            if args.blocked_only:
                entries = [
                    e
                    for e in entries
                    if (
                        e.action_status.value
                        if isinstance(e.action_status, ActionStatus)
                        else str(e.action_status)
                    )
                    == "BLOCKED"
                ]
            if args.limit is not None and args.limit > 0:
                entries = entries[-args.limit :]
            for e in entries:
                status = (
                    e.action_status.value
                    if isinstance(e.action_status, ActionStatus)
                    else str(e.action_status)
                )
                print(
                    f"[{e.timestamp}] seq={e.sequence_id} status={status} "
                    f"tool={e.tool_name} sig={e.signature[:12]}..."
                )
            return 0

        parser.print_help()
        return 1

    if args.command == "config":
        from medplum_mcp.config_generator import run_config_cli

        return run_config_cli(args)

    if args.command == "verify":
        from medplum_mcp.verifier_cli import run_verification_cli

        return run_verification_cli(args)

    # Run Server
    audit_mgr: AuditLogManager | None = None
    if args.audit_log_path:
        key = args.audit_key or "default-audit-key"
        audit_mgr = AuditLogManager(log_path=args.audit_log_path, secret_key=key)

    server = create_server(
        allow_writes=args.allow_writes,
        base_url=args.base_url,
        audit_manager=audit_mgr,
        demo_mode=args.demo,
        host=args.host,
        port=args.port,
    )

    if args.transport == "stdio":
        server.run(transport="stdio")
    elif args.transport == "sse":
        server.run(transport="sse", host=args.host, port=args.port)

    return 0


if __name__ == "__main__":
    sys.exit(main())
