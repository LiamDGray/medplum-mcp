"""Tests for Step 7: Documentation suite, formal specifications, and interactive demo console."""

import re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent


class TestDocumentationSuite:
    """Verifies README.md, ARCHITECTURE.md, and whitepaper.md meet enterprise standards."""

    def test_readme_exists_and_meets_standards(self) -> None:
        readme_path = REPO_ROOT / "README.md"
        assert readme_path.exists(), "README.md must exist in repository root"
        content = readme_path.read_text(encoding="utf-8")
        assert len(content) > 2000, "README.md must be comprehensive"

        # Check for status badges
        assert "[![" in content or "![" in content, "README.md must contain status badges"
        assert "badge" in content.lower() or "shields.io" in content or "svg" in content

        # Check links to key docs and specs
        assert "docs/ARCHITECTURE.md" in content or "docs/ARCHITECTURE" in content
        assert "docs/whitepaper.md" in content or "docs/whitepaper" in content
        assert "formal/fhir_safety.tla" in content
        assert "docs/demo/index.html" in content or "docs/demo" in content

        # Check for architectural diagrams / sections
        assert "```mermaid" in content or "Architecture" in content
        assert "HAMCP" in content or "Zero Unauthorized Prescriptions" in content
        assert "Tool Reference" in content or "Tools" in content

        # Quality check: No unsubstantiated hyperbolic buzzwords
        banned_hyperbole = [
            "insanely great",
            "world-class",
            "revolutionary",
            "miraculous",
            "game-changing",
            "mind-blowing",
            "paradigm shift",
            "ultimate",
            "2026 ultimate",
        ]
        for term in banned_hyperbole:
            assert term not in content.lower(), f"README.md contains banned hyperbole: {term!r}"

        # Soak test duration quality: do not boast about 93 minutes
        assert "93" not in content, "README.md should not reference 93 minutes soak test"

    def test_architecture_doc_exists_and_substantial(self) -> None:
        arch_path = REPO_ROOT / "docs" / "ARCHITECTURE.md"
        assert arch_path.exists(), "docs/ARCHITECTURE.md must exist"
        content = arch_path.read_text(encoding="utf-8")
        msg = f"docs/ARCHITECTURE.md must be substantial (>3000 bytes), got {len(content)}"
        assert len(content) >= 3000, msg

        # Soak test duration quality: do not boast about 93 minutes
        assert "93" not in content, "docs/ARCHITECTURE.md should not reference 93 minutes soak test"

        # Quality check: No unsubstantiated hyperbolic buzzwords
        banned_hyperbole = [
            "ultimate",
            "2026 ultimate",
            "revolutionary",
            "game-changing",
        ]
        for term in banned_hyperbole:
            msg = f"docs/ARCHITECTURE.md contains banned hyperbole: {term!r}"
            assert term not in content.lower(), msg

        # Verify 5 HAMCP invariant pillars applied to HL7 FHIR
        assert "Invariant" in content or "Pillars" in content
        assert "Zero Unauthorized" in content or "Prescription" in content
        assert "HMAC" in content or "Audit" in content
        assert "Token Diet" in content or "Distillation" in content
        assert "St. Jude" in content or "Sandbox" in content or "synthetic" in content.lower()

    def test_whitepaper_doc_exists_and_substantial(self) -> None:
        wp_path = REPO_ROOT / "docs" / "whitepaper.md"
        assert wp_path.exists(), "docs/whitepaper.md must exist"
        content = wp_path.read_text(encoding="utf-8")
        msg = f"docs/whitepaper.md must be substantial (>3000 bytes), got {len(content)}"
        assert len(content) >= 3000, msg

        # Verify required title & formal verification content
        assert (
            "Deterministic Safety Invariants and Cryptographic Auditability in Clinical AI"
            in content
            or "Proving Zero Unauthorized Prescriptions" in content
        )
        assert "TLA+" in content or "TLA" in content
        assert "Typestate" in content or "typestate" in content or "affine" in content.lower()
        assert "Inductive" in content or "SMT" in content or "Z3" in content


class TestFormalSpecifications:
    """Verifies TLA+ formal specifications."""

    def test_tla_spec_exists_and_valid(self) -> None:
        tla_path = REPO_ROOT / "formal" / "fhir_safety.tla"
        assert tla_path.exists(), "formal/fhir_safety.tla must exist"
        content = tla_path.read_text(encoding="utf-8")
        assert len(content) > 500, "TLA+ spec must be complete"

        # Check required TLA+ constructs
        assert "---- MODULE fhir_safety ----" in content
        assert "Init" in content
        assert "Next" in content
        assert "Spec" in content
        assert "ZeroUnauthorizedPrescription" in content or "SafetyInvariant" in content
        assert "====" in content


class TestInteractiveDemoConsole:
    """Verifies self-contained, offline interactive HTML demo console."""

    def test_demo_html_exists_and_offline(self) -> None:
        html_path = REPO_ROOT / "docs" / "demo" / "index.html"
        assert html_path.exists(), "docs/demo/index.html must exist"
        content = html_path.read_text(encoding="utf-8")
        msg = f"docs/demo/index.html must be substantial (>5000 bytes), got {len(content)}"
        assert len(content) >= 5000, msg

        # Zero external CDN script tags (100% offline-functional)
        assert not re.search(r'<script[^>]+src=["\'](http:|https:|\/\/)', content, re.IGNORECASE), (
            "docs/demo/index.html must not use external script CDNs"
        )
        assert not re.search(r'<link[^>]+href=["\'](http:|https:|\/\/)', content, re.IGNORECASE), (
            "docs/demo/index.html must not use external font/css CDNs"
        )

    def test_demo_html_required_sections_and_agents(self) -> None:
        html_path = REPO_ROOT / "docs" / "demo" / "index.html"
        assert html_path.exists()
        content = html_path.read_text(encoding="utf-8")

        # 4 required sections / components
        assert "token-diet-simulator" in content
        assert "safety-gate-simulator" in content
        assert "config-exporter" in content
        assert "verification-explorer" in content

        # Token Diet tiers and sizes
        assert "285" in content or "Raw" in content
        assert "Compact" in content
        assert "Standard" in content
        assert "Executive" in content

        # Safety Gate elements
        assert "403" in content or "INTERCEPTED" in content
        assert "HMAC" in content or "flight recorder" in content.lower()

        # All 7 agent clients represented in config exporter
        required_agents = [
            "Claude Desktop",
            "Claude Code",
            "Cursor",
            "Windsurf",
            "Pi Agent",
            "Hermes Agent",
            "Codex CLI",
        ]
        for agent in required_agents:
            assert agent in content, f"Demo console must contain config exporter for {agent}"

        # Verification explorer components
        assert "Z3" in content or "SMT" in content
        assert "TLA+" in content or "fhir_safety" in content
