"""High-Assurance HL7 FHIR Model Context Protocol (MCP) server for Medplum."""

from medplum_mcp.audit import (
    GENESIS_PREV_SIGNATURE,
    ActionStatus,
    AuditEntry,
    AuditLogManager,
    compute_entry_signature,
    compute_payload_digest,
    verify_audit_log,
)
from medplum_mcp.benchmarks import (
    SYNTHETIC_FHIR_FIXTURES,
    AggregateBenchmarkReport,
    BenchmarkResult,
    estimate_tokens,
    run_fhir_benchmarks,
)
from medplum_mcp.client import MedplumClient
from medplum_mcp.config_generator import (
    DEFAULT_SSE_URL,
    SUPPORTED_CLIENTS,
    generate_all_configs,
    generate_claude_code_command,
    generate_config,
    generate_server_entry,
    get_default_config_path,
    install_all_configs,
    install_config,
    merge_config,
    render_config_string,
    run_config_cli,
)
from medplum_mcp.demo import ClinicalSandbox
from medplum_mcp.fsm import (
    INITIAL_STATES,
    TRANSITION_MAPS,
    AllergyIntoleranceLifecycle,
    ClaimLifecycle,
    DiagnosticReportLifecycle,
    MedicationRequestLifecycle,
    ObservationLifecycle,
    compute_reachable_states,
    validate_transition,
)
from medplum_mcp.mock_server import MedplumMockServer
from medplum_mcp.safety import (
    FORBIDDEN_CLINICAL_STATUSES,
    SafetyInvariantViolation,
    assert_write_permitted,
    extract_statuses,
    normalize_status,
)
from medplum_mcp.server import (
    AuditedMCPServer,
    build_parser,
    create_parser,
    create_server,
    main,
)
from medplum_mcp.token_diet import (
    DetailLevel,
    distill_fhir_bundle,
    distill_fhir_resource,
    parse_detail_level,
)
from medplum_mcp.vault import (
    SecretString,
    VaultResolutionError,
    resolve_1password,
    resolve_aws_secrets,
    resolve_env,
    resolve_hashicorp_vault,
    resolve_secret,
)
from medplum_mcp.verifier_cli import (
    VerificationReport,
    check_homoglyph_immunity,
    run_verification,
    run_verification_cli,
    verify_audit_ledger,
)

__version__ = "0.1.0"

__all__ = [
    # Safety Gate & Invariants
    "FORBIDDEN_CLINICAL_STATUSES",
    "SafetyInvariantViolation",
    "assert_write_permitted",
    "extract_statuses",
    "normalize_status",
    # FSM Lifecycles
    "MedicationRequestLifecycle",
    "AllergyIntoleranceLifecycle",
    "ObservationLifecycle",
    "DiagnosticReportLifecycle",
    "ClaimLifecycle",
    "INITIAL_STATES",
    "TRANSITION_MAPS",
    "validate_transition",
    "compute_reachable_states",
    # HIPAA Cryptographic Audit Flight Recorder
    "GENESIS_PREV_SIGNATURE",
    "ActionStatus",
    "AuditEntry",
    "AuditLogManager",
    "compute_payload_digest",
    "compute_entry_signature",
    "verify_audit_log",
    # Zero-Trust Multi-Vault Resolution
    "SecretString",
    "VaultResolutionError",
    "resolve_env",
    "resolve_1password",
    "resolve_aws_secrets",
    "resolve_hashicorp_vault",
    "resolve_secret",
    # 3-Tier FHIR Context Distillation Engine
    "DetailLevel",
    "parse_detail_level",
    "distill_fhir_resource",
    "distill_fhir_bundle",
    # Token Distillation Benchmark Suite
    "SYNTHETIC_FHIR_FIXTURES",
    "estimate_tokens",
    "BenchmarkResult",
    "AggregateBenchmarkReport",
    "run_fhir_benchmarks",
    # In-Memory Clinical Sandbox, Client, and Mock Server
    "ClinicalSandbox",
    "MedplumClient",
    "MedplumMockServer",
    # FastMCP Clinical Server & CLI
    "AuditedMCPServer",
    "create_server",
    "build_parser",
    "create_parser",
    "main",
    # Universal Multi-Agent Config Generator
    "SUPPORTED_CLIENTS",
    "DEFAULT_SSE_URL",
    "get_default_config_path",
    "generate_server_entry",
    "generate_config",
    "generate_all_configs",
    "generate_claude_code_command",
    "render_config_string",
    "merge_config",
    "install_config",
    "install_all_configs",
    "run_config_cli",
    # Automated Cryptographic Verification Engine
    "VerificationReport",
    "check_homoglyph_immunity",
    "verify_audit_ledger",
    "run_verification",
    "run_verification_cli",
]
