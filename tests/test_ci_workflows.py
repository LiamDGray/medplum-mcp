from pathlib import Path

import yaml


def test_ci_workflow_structure_and_jobs() -> None:
    repo_root = Path(__file__).resolve().parent.parent
    ci_path = repo_root / ".github" / "workflows" / "ci.yml"
    assert ci_path.exists(), f"Workflow file {ci_path} does not exist"

    content = ci_path.read_text(encoding="utf-8")
    data = yaml.safe_load(content)

    assert "name" in data
    assert "on" in data
    assert "jobs" in data

    jobs = data["jobs"]
    expected_jobs = {
        "rust-checks",
        "python-checks",
        "fuzz-smoke",
        "miri-ub-verification",
        "kani-proofs",
        "soak-smoke",
        "mcp-conformance",
    }

    missing = expected_jobs - set(jobs.keys())
    assert expected_jobs.issubset(set(jobs.keys())), f"Missing jobs in CI workflow: {missing}"

    # Verify rust-checks job
    rust_job = jobs["rust-checks"]
    rust_steps = [s.get("run", "") for s in rust_job.get("steps", []) if "run" in s]
    assert any("cargo fmt" in s for s in rust_steps)
    assert any("cargo clippy" in s for s in rust_steps)
    assert any("cargo test" in s for s in rust_steps)

    # Verify python-checks job
    py_job = jobs["python-checks"]
    py_steps = [s.get("run", "") for s in py_job.get("steps", []) if "run" in s]
    assert any("ruff check" in s for s in py_steps)
    assert any("mypy" in s for s in py_steps)
    assert any("pytest" in s for s in py_steps)

    # Verify fuzz-smoke job
    fuzz_job = jobs["fuzz-smoke"]
    fuzz_steps = [s.get("run", "") for s in fuzz_job.get("steps", []) if "run" in s]
    assert any("cargo fuzz" in s or "run_fuzz_checks.sh" in s for s in fuzz_steps)

    # Verify miri-ub-verification job
    miri_job = jobs["miri-ub-verification"]
    miri_steps = [s.get("run", "") for s in miri_job.get("steps", []) if "run" in s]
    assert any("miri" in s or "run_miri_checks.sh" in s for s in miri_steps)

    # Verify kani-proofs job
    kani_job = jobs["kani-proofs"]
    kani_steps = [s.get("run", "") + s.get("uses", "") for s in kani_job.get("steps", [])]
    assert any("kani" in s or "run_kani_checks.sh" in s for s in kani_steps)

    # Verify soak-smoke job
    soak_job = jobs["soak-smoke"]
    soak_steps = [s.get("run", "") for s in soak_job.get("steps", []) if "run" in s]
    assert any("soak" in s for s in soak_steps)

    # Verify mcp-conformance job
    conf_job = jobs["mcp-conformance"]
    conf_steps = [s.get("run", "") for s in conf_job.get("steps", []) if "run" in s]
    assert any("conformance" in s or "run_conformance.sh" in s for s in conf_steps)
