"""Tests for 3-Tier FHIR Context Distillation Engine and Benchmark Suite."""

import pytest

from medplum_mcp.benchmarks import (
    SYNTHETIC_FHIR_FIXTURES,
    AggregateBenchmarkReport,
    BenchmarkResult,
    estimate_tokens,
    run_fhir_benchmarks,
)
from medplum_mcp.token_diet import (
    DetailLevel,
    distill_fhir_bundle,
    distill_fhir_resource,
    parse_detail_level,
)


class TestDetailLevelParsing:
    """Test DetailLevel enum and parsing logic."""

    def test_valid_detail_levels(self) -> None:
        assert parse_detail_level("compact") == DetailLevel.COMPACT
        assert parse_detail_level("COMPACT") == DetailLevel.COMPACT
        assert parse_detail_level("standard") == DetailLevel.STANDARD
        assert parse_detail_level("Standard") == DetailLevel.STANDARD
        assert parse_detail_level("executive") == DetailLevel.EXECUTIVE
        assert parse_detail_level("EXECUTIVE") == DetailLevel.EXECUTIVE
        assert parse_detail_level(DetailLevel.COMPACT) == DetailLevel.COMPACT

    def test_default_detail_level(self) -> None:
        assert parse_detail_level() == DetailLevel.STANDARD

    def test_invalid_detail_level_raises(self) -> None:
        with pytest.raises(ValueError, match="Invalid detail level"):
            parse_detail_level("ultra")
        with pytest.raises(ValueError, match="Invalid detail level"):
            parse_detail_level("")
        with pytest.raises(ValueError, match="Invalid detail level"):
            parse_detail_level(None)  # type: ignore[arg-type]


class TestResourceDistillation:
    """Test distillation of all 8 core FHIR resource types across 3 tiers."""

    def test_patient_distillation(self) -> None:
        patient_fixture = SYNTHETIC_FHIR_FIXTURES["Patient"]

        # Compact
        compact = distill_fhir_resource(patient_fixture, detail_level="compact")
        assert compact["resourceType"] == "Patient"
        assert compact["id"] == patient_fixture["id"]
        assert "Jane" in compact["name"] and "Smith" in compact["name"]
        assert "MRN-12345" in compact["identifier"]
        assert compact["gender"] == "female"
        assert compact["birthDate"] == "1980-01-15"
        assert "telecom" not in compact  # Excluded in compact
        assert "meta" not in compact
        assert "extension" not in compact

        # Standard
        standard = distill_fhir_resource(patient_fixture, detail_level="standard")
        assert standard["resourceType"] == "Patient"
        assert standard["id"] == patient_fixture["id"]
        assert "Jane" in standard["name"] and "Smith" in standard["name"]
        assert "MRN-12345" in standard["identifier"]
        assert standard["gender"] == "female"
        assert standard["birthDate"] == "1980-01-15"
        assert "telecom" in standard
        assert len(standard["telecom"]) >= 1

        # Executive
        executive = distill_fhir_resource(patient_fixture, detail_level="executive")
        assert executive["resourceType"] == "Patient"
        assert executive["id"] == patient_fixture["id"]
        assert "Jane" in executive["name"] and "Smith" in executive["name"]
        assert "MRN-12345" in executive["identifier"]
        assert "active" in executive or "status" in executive or "summary" in executive

    def test_observation_distillation(self) -> None:
        obs_fixture = SYNTHETIC_FHIR_FIXTURES["Observation"]

        # Compact: Primary IDs, resourceType, codes/labels, and primary value/status
        compact = distill_fhir_resource(obs_fixture, detail_level="compact")
        assert compact["resourceType"] == "Observation"
        assert compact["id"] == obs_fixture["id"]
        assert "Glucose" in compact["code"]
        assert compact["value"] == 140
        assert compact["unit"] == "mg/dL"
        assert compact["status"] == "final"
        assert "referenceRange" not in compact

        # Standard: Clinical details, performers, dates, reference ranges, interpretation flags
        standard = distill_fhir_resource(obs_fixture, detail_level="standard")
        assert standard["resourceType"] == "Observation"
        assert "Glucose" in standard["code"]
        assert standard["value"] == 140
        assert standard["unit"] == "mg/dL"
        assert standard["status"] == "final"
        assert "effectiveDateTime" in standard
        assert "referenceRange" in standard
        assert "interpretation" in standard
        assert "performer" in standard

        # Executive: High-level summary, clinical status, critical flag indicators
        executive = distill_fhir_resource(obs_fixture, detail_level="executive")
        assert executive["resourceType"] == "Observation"
        assert "Glucose" in executive["code"]
        assert (
            "High" in str(executive.get("interpretation")) or executive.get("is_critical") is True
        )
        assert executive["status"] == "final"

    def test_condition_distillation(self) -> None:
        cond_fixture = SYNTHETIC_FHIR_FIXTURES["Condition"]

        # Compact
        compact = distill_fhir_resource(cond_fixture, detail_level="compact")
        assert compact["resourceType"] == "Condition"
        assert "Diabetes" in compact["code"]
        assert compact["clinicalStatus"] == "active"
        assert "verificationStatus" not in compact

        # Standard: Clinical details, dates, verification
        standard = distill_fhir_resource(cond_fixture, detail_level="standard")
        assert standard["resourceType"] == "Condition"
        assert "Diabetes" in standard["code"]
        assert standard["clinicalStatus"] == "active"
        assert standard["verificationStatus"] == "confirmed"
        assert "onsetDateTime" in standard

        # Executive: High-level summary, critical indicators
        executive = distill_fhir_resource(cond_fixture, detail_level="executive")
        assert executive["resourceType"] == "Condition"
        assert "Diabetes" in executive["code"]
        assert executive["clinicalStatus"] == "active"

    def test_medication_request_distillation(self) -> None:
        med_fixture = SYNTHETIC_FHIR_FIXTURES["MedicationRequest"]

        # Compact
        compact = distill_fhir_resource(med_fixture, detail_level="compact")
        assert compact["resourceType"] == "MedicationRequest"
        assert "Metformin" in compact["medication"]
        assert compact["status"] == "active"
        assert "dosageInstruction" not in compact

        # Standard
        standard = distill_fhir_resource(med_fixture, detail_level="standard")
        assert standard["resourceType"] == "MedicationRequest"
        assert "Metformin" in standard["medication"]
        assert standard["status"] == "active"
        assert standard["intent"] == "order"
        assert "dosageInstruction" in standard

        # Executive
        executive = distill_fhir_resource(med_fixture, detail_level="executive")
        assert executive["resourceType"] == "MedicationRequest"
        assert "Metformin" in executive["medication"]
        assert executive["status"] == "active"
        assert executive["intent"] == "order"

    def test_allergy_intolerance_distillation(self) -> None:
        allergy_fixture = SYNTHETIC_FHIR_FIXTURES["AllergyIntolerance"]

        # Compact
        compact = distill_fhir_resource(allergy_fixture, detail_level="compact")
        assert compact["resourceType"] == "AllergyIntolerance"
        assert "Penicillin" in compact["code"]
        assert compact["clinicalStatus"] == "active"
        assert compact["criticality"] == "high"
        assert "reaction" not in compact

        # Standard
        standard = distill_fhir_resource(allergy_fixture, detail_level="standard")
        assert standard["resourceType"] == "AllergyIntolerance"
        assert "Penicillin" in standard["code"]
        assert standard["clinicalStatus"] == "active"
        assert standard["verificationStatus"] == "confirmed"
        assert standard["criticality"] == "high"
        assert "reaction" in standard

        # Executive
        executive = distill_fhir_resource(allergy_fixture, detail_level="executive")
        assert executive["resourceType"] == "AllergyIntolerance"
        assert "Penicillin" in executive["code"]
        assert executive["criticality"] == "high"
        assert executive.get("is_critical") is True

    def test_diagnostic_report_distillation(self) -> None:
        report_fixture = SYNTHETIC_FHIR_FIXTURES["DiagnosticReport"]

        # Compact
        compact = distill_fhir_resource(report_fixture, detail_level="compact")
        assert compact["resourceType"] == "DiagnosticReport"
        assert "Metabolic" in compact["code"]
        assert compact["status"] == "final"
        assert "conclusion" not in compact

        # Standard
        standard = distill_fhir_resource(report_fixture, detail_level="standard")
        assert standard["resourceType"] == "DiagnosticReport"
        assert "Metabolic" in standard["code"]
        assert standard["status"] == "final"
        assert "effectiveDateTime" in standard
        assert "category" in standard
        assert "conclusion" in standard

        # Executive
        executive = distill_fhir_resource(report_fixture, detail_level="executive")
        assert executive["resourceType"] == "DiagnosticReport"
        assert "Metabolic" in executive["code"]
        assert executive["status"] == "final"
        assert "conclusion" in executive

    def test_encounter_distillation(self) -> None:
        enc_fixture = SYNTHETIC_FHIR_FIXTURES["Encounter"]

        # Compact
        compact = distill_fhir_resource(enc_fixture, detail_level="compact")
        assert compact["resourceType"] == "Encounter"
        assert compact["status"] == "finished"
        assert "class" in compact
        assert "type" in compact
        assert "period" not in compact

        # Standard
        standard = distill_fhir_resource(enc_fixture, detail_level="standard")
        assert standard["resourceType"] == "Encounter"
        assert standard["status"] == "finished"
        assert "class" in standard
        assert "type" in standard
        assert "period" in standard
        assert "reasonCode" in standard

        # Executive
        executive = distill_fhir_resource(enc_fixture, detail_level="executive")
        assert executive["resourceType"] == "Encounter"
        assert executive["status"] == "finished"
        assert "type" in executive

    def test_care_plan_distillation(self) -> None:
        cp_fixture = SYNTHETIC_FHIR_FIXTURES["CarePlan"]

        # Compact
        compact = distill_fhir_resource(cp_fixture, detail_level="compact")
        assert compact["resourceType"] == "CarePlan"
        assert "Diabetes" in compact["title"]
        assert compact["status"] == "active"
        assert compact["intent"] == "plan"
        assert "activities" not in compact

        # Standard
        standard = distill_fhir_resource(cp_fixture, detail_level="standard")
        assert standard["resourceType"] == "CarePlan"
        assert "Diabetes" in standard["title"]
        assert standard["status"] == "active"
        assert standard["intent"] == "plan"
        assert "period" in standard
        assert "activities" in standard
        assert len(standard["activities"]) >= 1

        # Executive
        executive = distill_fhir_resource(cp_fixture, detail_level="executive")
        assert executive["resourceType"] == "CarePlan"
        assert "Diabetes" in executive["title"]
        assert executive["status"] == "active"
        assert "activity_count" in executive or "activities" in executive

    def test_bundle_distillation(self) -> None:
        bundle_fixture = SYNTHETIC_FHIR_FIXTURES["Bundle"]

        # Test both distill_fhir_resource and distill_fhir_bundle
        for tier in ["compact", "standard", "executive"]:
            distilled_res = distill_fhir_resource(bundle_fixture, detail_level=tier)
            distilled_bundle = distill_fhir_bundle(bundle_fixture, detail_level=tier)

            assert distilled_res["resourceType"] == "Bundle"
            assert distilled_bundle["resourceType"] == "Bundle"
            assert distilled_bundle["total"] == bundle_fixture["total"]
            assert len(distilled_bundle["entry"]) == len(bundle_fixture["entry"])

            # Verify entries inside bundle are distilled
            first_entry_resource = distilled_bundle["entry"][0]["resource"]
            assert "meta" not in first_entry_resource
            assert "text" not in first_entry_resource
            assert "extension" not in first_entry_resource


class TestTokenDietReductionMetrics:
    """Assert reduction percentage targets across tiers on realistic synthetic fixtures."""

    @pytest.mark.parametrize(
        "res_type",
        [
            "Patient",
            "Observation",
            "Condition",
            "MedicationRequest",
            "AllergyIntolerance",
            "DiagnosticReport",
            "Encounter",
            "CarePlan",
            "Bundle",
        ],
    )
    def test_fixture_tier_reduction_targets(self, res_type: str) -> None:
        raw = SYNTHETIC_FHIR_FIXTURES[res_type]
        raw_tokens = estimate_tokens(raw)

        compact = distill_fhir_resource(raw, detail_level="compact")
        compact_tokens = estimate_tokens(compact)
        compact_red = (1.0 - (compact_tokens / raw_tokens)) * 100.0

        standard = distill_fhir_resource(raw, detail_level="standard")
        standard_tokens = estimate_tokens(standard)
        standard_red = (1.0 - (standard_tokens / raw_tokens)) * 100.0

        executive = distill_fhir_resource(raw, detail_level="executive")
        executive_tokens = estimate_tokens(executive)
        executive_red = (1.0 - (executive_tokens / raw_tokens)) * 100.0

        # Targets:
        # compact > 90%
        # standard > 80%
        # executive > 85%
        assert compact_red >= 90.0, f"{res_type} compact reduction {compact_red:.1f}% < 90%"
        assert standard_red >= 80.0, f"{res_type} standard reduction {standard_red:.1f}% < 80%"
        assert executive_red >= 85.0, f"{res_type} executive reduction {executive_red:.1f}% < 85%"


class TestBenchmarkSuite:
    """Test full benchmark suite execution."""

    def test_run_fhir_benchmarks(self) -> None:
        report = run_fhir_benchmarks()

        assert isinstance(report, AggregateBenchmarkReport)
        assert len(report.results) == len(SYNTHETIC_FHIR_FIXTURES)

        # Aggregate reduction >= 85%
        assert report.aggregate_compact_reduction_pct >= 90.0
        assert report.aggregate_standard_reduction_pct >= 80.0
        assert report.aggregate_executive_reduction_pct >= 85.0
        assert report.overall_reduction_pct >= 85.0

        for _name, res in report.results.items():
            assert isinstance(res, BenchmarkResult)
            assert res.compact_reduction_pct >= 90.0
            assert res.standard_reduction_pct >= 80.0
            assert res.executive_reduction_pct >= 85.0
