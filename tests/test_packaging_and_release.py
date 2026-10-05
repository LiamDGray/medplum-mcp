from pathlib import Path

import tomllib  # type: ignore[import-not-found]
import yaml


def test_cargo_crates_io_metadata_and_readmes() -> None:
    repo_root = Path(__file__).resolve().parent.parent
    crate_dirs = [
        repo_root / "crates" / "medplum-mcp-core",
        repo_root / "crates" / "medplum-mcp-server",
        repo_root / "crates" / "medplum-mcp-cli",
    ]

    required_fields = [
        "name",
        "version",
        "edition",
        "description",
        "license",
        "repository",
        "homepage",
        "readme",
        "keywords",
        "categories",
    ]

    for crate_dir in crate_dirs:
        cargo_path = crate_dir / "Cargo.toml"
        assert cargo_path.exists(), f"Cargo.toml missing in {crate_dir}"
        cargo_data = tomllib.loads(cargo_path.read_text(encoding="utf-8"))
        pkg = cargo_data.get("package", {})

        for field in required_fields:
            assert field in pkg, f"Field '{field}' missing in package {crate_dir.name}"
            assert pkg[field], f"Field '{field}' is empty in {crate_dir.name}"

        # Verify README.md exists and is non-empty
        readme_path = crate_dir / pkg["readme"]
        assert readme_path.exists(), f"README {readme_path} does not exist"
        assert len(readme_path.read_text(encoding="utf-8").strip()) > 50

        # Check internal dependencies specify version
        deps = cargo_data.get("dependencies", {})
        for dep_name in ["medplum-mcp-core", "medplum-mcp-server"]:
            if dep_name in deps and isinstance(deps[dep_name], dict):
                assert "version" in deps[dep_name], (
                    f"Dependency {dep_name} in {crate_dir.name} must specify version for crates.io"
                )


def test_release_ci_workflow_structure() -> None:
    repo_root = Path(__file__).resolve().parent.parent
    release_path = repo_root / ".github" / "workflows" / "release.yml"
    assert release_path.exists(), f"Workflow file {release_path} does not exist"

    content = release_path.read_text(encoding="utf-8")
    data = yaml.safe_load(content)

    assert "name" in data
    assert "on" in data
    assert "jobs" in data

    jobs = data["jobs"]
    assert "release-binaries" in jobs or "build-binaries" in jobs
    release_job = jobs.get("release-binaries") or jobs.get("build-binaries")
    assert release_job is not None

    strategy = release_job.get("strategy", {})
    matrix = strategy.get("matrix", {})
    targets = matrix.get("target", [])
    if not targets and "include" in matrix:
        targets = [item.get("target") for item in matrix["include"] if "target" in item]
    expected_targets = {
        "x86_64-unknown-linux-gnu",
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
    }
    missing = expected_targets - set(targets)
    assert expected_targets.issubset(set(targets)), f"Missing release targets: {missing}"
