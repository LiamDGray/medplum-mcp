"""Tests for Universal Multi-Agent Config Generator (medplum-mcp config)."""

from __future__ import annotations

import json
from pathlib import Path

import pytest
import yaml

from medplum_mcp.config_generator import (
    SUPPORTED_CLIENTS,
    generate_all_configs,
    generate_claude_code_command,
    generate_config,
    get_default_config_path,
    install_all_configs,
    install_config,
    merge_config,
    render_config_string,
)
from medplum_mcp.server import main


def test_supported_clients_list() -> None:
    """Assert all 7 required agent clients are supported."""
    expected = {
        "claude-desktop",
        "claude-code",
        "cursor",
        "windsurf",
        "pi-agent",
        "hermes-agent",
        "codex-cli",
    }
    assert set(SUPPORTED_CLIENTS) == expected


@pytest.mark.parametrize(
    "client",
    ["claude-desktop", "claude-code", "cursor", "windsurf", "pi-agent"],
)
def test_generate_config_json_clients_stdio(client: str) -> None:
    """Test standard I/O config dictionary generation for JSON-based clients."""
    cfg = generate_config(client=client, demo=False, transport="stdio")
    assert "mcpServers" in cfg
    assert "medplum" in cfg["mcpServers"]
    medplum = cfg["mcpServers"]["medplum"]
    assert medplum["command"] == "medplum-mcp"
    assert medplum["args"] == []
    assert "url" not in medplum


def test_generate_config_hermes_stdio() -> None:
    """Test hermes-agent config dictionary generation in stdio mode."""
    cfg = generate_config(client="hermes-agent", demo=False, transport="stdio")
    assert "mcp_servers" in cfg
    assert "medplum" in cfg["mcp_servers"]
    medplum = cfg["mcp_servers"]["medplum"]
    assert medplum["command"] == "medplum-mcp"
    assert medplum["args"] == []
    assert "url" not in medplum


def test_generate_config_codex_stdio() -> None:
    """Test codex-cli config dictionary generation in stdio mode."""
    cfg = generate_config(client="codex-cli", demo=False, transport="stdio")
    assert "tools" in cfg
    assert "mcp" in cfg["tools"]
    assert isinstance(cfg["tools"]["mcp"], list)
    medplum = next(
        (item for item in cfg["tools"]["mcp"] if item.get("name") == "medplum"),
        None,
    )
    assert medplum is not None
    assert medplum["command"] == "medplum-mcp"
    assert medplum["args"] == []
    assert "url" not in medplum


@pytest.mark.parametrize("client", SUPPORTED_CLIENTS)
def test_generate_config_demo_mode(client: str) -> None:
    """Test that --demo flag adds '--demo' to server arguments for all clients."""
    cfg = generate_config(client=client, demo=True, transport="stdio")
    if client in {"claude-desktop", "claude-code", "cursor", "windsurf", "pi-agent"}:
        assert cfg["mcpServers"]["medplum"]["args"] == ["--demo"]
    elif client == "hermes-agent":
        assert cfg["mcp_servers"]["medplum"]["args"] == ["--demo"]
    elif client == "codex-cli":
        item = cfg["tools"]["mcp"][0]
        assert item["args"] == ["--demo"]


@pytest.mark.parametrize("client", SUPPORTED_CLIENTS)
def test_generate_config_sse_mode(client: str) -> None:
    """Test that SSE mode generates URL instead of command/args for all clients."""
    cfg = generate_config(
        client=client,
        demo=False,
        transport="sse",
        url="http://localhost:8000/sse",
    )
    if client in {"claude-desktop", "claude-code", "cursor", "windsurf", "pi-agent"}:
        medplum = cfg["mcpServers"]["medplum"]
        assert medplum["url"] == "http://localhost:8000/sse"
        assert "command" not in medplum
        assert "args" not in medplum
    elif client == "hermes-agent":
        medplum = cfg["mcp_servers"]["medplum"]
        assert medplum["url"] == "http://localhost:8000/sse"
        assert "command" not in medplum
        assert "args" not in medplum
    elif client == "codex-cli":
        item = cfg["tools"]["mcp"][0]
        assert item["name"] == "medplum"
        assert item["url"] == "http://localhost:8000/sse"
        assert "command" not in item
        assert "args" not in item


def test_generate_all_configs() -> None:
    """Test generating full combined config dictionary across all clients."""
    all_cfg = generate_all_configs(demo=True, transport="stdio")
    assert set(all_cfg.keys()) == set(SUPPORTED_CLIENTS)
    assert all_cfg["claude-desktop"]["mcpServers"]["medplum"]["args"] == ["--demo"]
    assert all_cfg["hermes-agent"]["mcp_servers"]["medplum"]["args"] == ["--demo"]


def test_generate_claude_code_command() -> None:
    """Test generation of claude mcp add shell command."""
    cmd_stdio = generate_claude_code_command(demo=False, transport="stdio")
    assert cmd_stdio == "claude mcp add medplum -- medplum-mcp"

    cmd_demo = generate_claude_code_command(demo=True, transport="stdio")
    assert cmd_demo == "claude mcp add medplum -- medplum-mcp --demo"

    cmd_sse = generate_claude_code_command(
        demo=False, transport="sse", url="http://localhost:8000/sse"
    )
    assert cmd_sse == "claude mcp add --transport sse medplum http://localhost:8000/sse"


def test_render_config_string() -> None:
    """Test rendering config dictionary to formatted JSON or YAML string."""
    cfg_json = generate_config("cursor", demo=True)
    rendered_json = render_config_string("cursor", cfg_json)
    parsed_json = json.loads(rendered_json)
    assert parsed_json["mcpServers"]["medplum"]["args"] == ["--demo"]

    cfg_yaml = generate_config("hermes-agent", demo=True)
    rendered_yaml = render_config_string("hermes-agent", cfg_yaml)
    parsed_yaml = yaml.safe_load(rendered_yaml)
    assert parsed_yaml["mcp_servers"]["medplum"]["args"] == ["--demo"]


def test_merge_config_non_destructive_json() -> None:
    """Test non-destructive merging for JSON clients preserving existing servers."""
    existing = json.dumps(
        {
            "mcpServers": {
                "sqlite": {"command": "sqlite-mcp", "args": ["data.db"]},
                "medplum": {"command": "old-medplum", "args": []},
            },
            "customSetting": True,
        }
    )
    new_cfg = generate_config("cursor", demo=True)
    data, text = merge_config("cursor", existing, new_cfg)

    assert data["customSetting"] is True
    assert "sqlite" in data["mcpServers"]
    assert data["mcpServers"]["sqlite"]["command"] == "sqlite-mcp"
    assert data["mcpServers"]["medplum"]["command"] == "medplum-mcp"
    assert data["mcpServers"]["medplum"]["args"] == ["--demo"]


def test_merge_config_non_destructive_yaml_hermes() -> None:
    """Test non-destructive merging for hermes-agent preserving other servers and root keys."""
    existing = yaml.safe_dump(
        {
            "model": "gpt-4",
            "mcp_servers": {
                "weather": {"command": "weather-mcp"},
            },
        }
    )
    new_cfg = generate_config("hermes-agent", demo=False)
    data, _ = merge_config("hermes-agent", existing, new_cfg)

    assert data["model"] == "gpt-4"
    assert "weather" in data["mcp_servers"]
    assert data["mcp_servers"]["medplum"]["command"] == "medplum-mcp"


def test_merge_config_non_destructive_yaml_codex() -> None:
    """Test non-destructive merging for codex-cli updating tool entry preserving others."""
    existing = yaml.safe_dump(
        {
            "tools": {
                "mcp": [
                    {"name": "fetch", "command": "fetch-mcp"},
                    {"name": "medplum", "command": "old-medplum"},
                ]
            }
        }
    )
    new_cfg = generate_config("codex-cli", demo=True)
    data, _ = merge_config("codex-cli", existing, new_cfg)

    mcp_list = data["tools"]["mcp"]
    assert len(mcp_list) == 2
    medplum_item = next(item for item in mcp_list if item["name"] == "medplum")
    assert medplum_item["args"] == ["--demo"]


def test_get_default_config_path(tmp_path: Path) -> None:
    """Test default config path resolution for each client with base_dir override."""
    p_desktop = get_default_config_path("claude-desktop", base_dir=tmp_path)
    assert p_desktop == tmp_path / ".config" / "Claude" / "claude_desktop_config.json"

    p_claude_code = get_default_config_path("claude-code", base_dir=tmp_path)
    assert p_claude_code == tmp_path / ".claude" / "mcp.json"

    p_cursor = get_default_config_path("cursor", base_dir=tmp_path)
    assert p_cursor == tmp_path / ".cursor" / "mcp.json"

    p_windsurf = get_default_config_path("windsurf", base_dir=tmp_path)
    assert p_windsurf == tmp_path / ".codeium" / "windsurf" / "mcp_config.json"

    p_pi = get_default_config_path("pi-agent", base_dir=tmp_path)
    assert p_pi == tmp_path / ".pi" / "agent" / "mcp.json"

    p_hermes = get_default_config_path("hermes-agent", base_dir=tmp_path)
    assert p_hermes == tmp_path / ".hermes" / "config.yaml"

    p_codex = get_default_config_path("codex-cli", base_dir=tmp_path)
    assert p_codex == tmp_path / ".codex" / "config.yaml"


def test_install_config_and_install_all_configs(tmp_path: Path) -> None:
    """Test writing and merging configs directly to disk."""
    dest = tmp_path / "custom" / "mcp.json"
    written = install_config("cursor", dest, demo=True)
    assert written.is_file()
    content = json.loads(written.read_text(encoding="utf-8"))
    assert content["mcpServers"]["medplum"]["args"] == ["--demo"]

    installed_all = install_all_configs(base_dir=tmp_path, demo=True)
    assert len(installed_all) == 7
    for _client, file_path in installed_all.items():
        assert file_path.is_file()


def test_cli_config_stdout(capsys: pytest.CaptureFixture[str]) -> None:
    """Test CLI subcommand 'medplum-mcp config --client cursor' prints valid JSON."""
    exit_code = main(["config", "--client", "cursor", "--demo"])
    assert exit_code == 0
    captured = capsys.readouterr()
    cfg = json.loads(captured.out)
    assert cfg["mcpServers"]["medplum"]["args"] == ["--demo"]


def test_cli_config_all_stdout(capsys: pytest.CaptureFixture[str]) -> None:
    """Test CLI subcommand 'medplum-mcp config --client all' prints dictionary of all clients."""
    exit_code = main(["config", "--client", "all", "--demo"])
    assert exit_code == 0
    captured = capsys.readouterr()
    cfg = json.loads(captured.out)
    assert set(cfg.keys()) == set(SUPPORTED_CLIENTS)


def test_cli_config_install(tmp_path: Path) -> None:
    """Test CLI subcommand with --install writes to output-dir."""
    exit_code = main(
        [
            "config",
            "--client",
            "cursor",
            "--install",
            "--output-dir",
            str(tmp_path),
            "--demo",
        ]
    )
    assert exit_code == 0
    expected_file = tmp_path / ".cursor" / "mcp.json"
    assert expected_file.is_file()
