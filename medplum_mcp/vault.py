"""Zero-Trust Multi-Vault Resolution and Zero-Leak Enclaves.

Provides cryptographic secret protection under HIPAA § 164.312(a)(2)(iv) and § 164.312(e)(2)(ii):
- Enclave masking of sensitive credentials (preventing leakage into logs or LLM context)
- Zero-trust multi-vault resolution across:
  - System environment variables (standard or env: prefix)
  - 1Password CLI (op://<vault>/<item>/<field>)
  - AWS Secrets Manager (arn:aws:secretsmanager:... or aws-secretsmanager:...)
  - HashiCorp Vault (vault://<path>#<field>)
"""

from __future__ import annotations

import hmac
import json
import os
import subprocess
from typing import Any

import requests


class VaultResolutionError(Exception):
    """Raised when secret retrieval or credential resolution fails."""


class SecretString:
    """Zero-leak enclave holding a sensitive secret or PHI encryption key.

    Overrides __repr__ and __str__ to unconditionally return '***', ensuring
    tokens and keys are never emitted in tracebacks, logs, or LLM context windows.
    Access to plaintext is gated behind explicit .get_secret_value() or .expose().
    """

    def __init__(self, value: str) -> None:
        self._value: str = value

    def get_secret_value(self) -> str:
        """Return the unmasked secret value."""
        return self._value

    def expose(self) -> str:
        """Alias for get_secret_value() providing API compatibility."""
        return self._value

    def __repr__(self) -> str:
        return "***"

    def __str__(self) -> str:
        return "***"

    def __format__(self, format_spec: str) -> str:
        return "***"

    def __eq__(self, other: object) -> bool:
        if isinstance(other, SecretString):
            return hmac.compare_digest(self._value, other._value)
        if isinstance(other, str):
            return hmac.compare_digest(self._value, other)
        return False

    def __hash__(self) -> int:
        return hash(self._value)

    def __bool__(self) -> bool:
        return bool(self._value)

    def __len__(self) -> int:
        return len(self._value)


def resolve_env(uri: str) -> SecretString:
    """Resolve a secret from an environment variable."""
    var_name = uri[4:] if uri.startswith("env:") else uri
    if not var_name:
        raise VaultResolutionError("Environment variable name cannot be empty")
    value = os.environ.get(var_name)
    if value is None:
        raise VaultResolutionError(f"Secret '{var_name}' not found in environment")
    return SecretString(value)


def resolve_1password(uri: str) -> SecretString:
    """Resolve a secret via 1Password CLI ('op read op://...')."""
    if not uri.startswith("op://"):
        raise VaultResolutionError(f"Invalid 1Password URI format: {uri}. Expected 'op://...'")
    try:
        proc = subprocess.run(
            ["op", "read", uri],
            capture_output=True,
            text=True,
            check=False,
        )
    except FileNotFoundError as exc:
        raise VaultResolutionError("1Password CLI ('op') not found on system PATH") from exc
    except Exception as exc:
        raise VaultResolutionError(f"Failed to execute 1Password CLI: {exc}") from exc

    if proc.returncode != 0:
        err_msg = proc.stderr.strip() or f"1Password CLI exited with code {proc.returncode}"
        raise VaultResolutionError(f"1Password resolution failed: {err_msg}")

    secret_val = proc.stdout.rstrip("\r\n")
    return SecretString(secret_val)


def resolve_aws_secrets(uri: str, *, client: Any = None) -> SecretString:
    """Resolve a secret from AWS Secrets Manager.

    Supports:
      - ARN with optional field key: arn:aws:secretsmanager:<region>:<account>:secret:<name>[:<key>]
      - Prefix URI syntax: aws-secretsmanager:<secret_id>[:<key>]
    """
    secret_id: str
    key: str | None = None

    if uri.startswith("arn:aws:secretsmanager:"):
        parts = uri.split(":")
        # Minimum valid ARN: arn:aws:secretsmanager:region:account:secret:name (7 parts)
        if len(parts) == 7:
            secret_id = uri
            key = None
        elif len(parts) >= 8:
            secret_id = ":".join(parts[:7])
            key = ":".join(parts[7:])
        else:
            raise VaultResolutionError(
                f"Invalid AWS Secrets Manager ARN format: {uri}. "
                "Expected at least 7 colon-separated segments."
            )
    elif uri.startswith("aws-secretsmanager:"):
        payload = uri[len("aws-secretsmanager:") :]
        if not payload:
            raise VaultResolutionError("AWS Secrets Manager URI missing secret ID")
        if ":" in payload:
            secret_id, key = payload.split(":", 1)
        else:
            secret_id, key = payload, None
    else:
        raise VaultResolutionError(
            f"Unsupported AWS Secrets Manager format: {uri}. "
            "Must start with 'arn:aws:secretsmanager:' or 'aws-secretsmanager:'"
        )

    if not secret_id:
        raise VaultResolutionError("AWS Secrets Manager secret ID cannot be empty")

    if client is None:
        try:
            import boto3  # type: ignore[import-untyped]

            client = boto3.client("secretsmanager")
        except ImportError as exc:
            raise VaultResolutionError(
                "boto3 package is required for AWS Secrets Manager resolution"
            ) from exc
        except Exception as exc:
            raise VaultResolutionError(
                f"Failed to initialize AWS Secrets Manager client: {exc}"
            ) from exc

    try:
        response: dict[str, Any] = client.get_secret_value(SecretId=secret_id)
    except Exception as exc:
        raise VaultResolutionError(f"Failed to retrieve AWS secret '{secret_id}': {exc}") from exc

    secret_str = response.get("SecretString")
    if secret_str is None:
        binary_val = response.get("SecretBinary")
        if binary_val is not None:
            if isinstance(binary_val, bytes):
                secret_str = binary_val.decode("utf-8")
            else:
                secret_str = str(binary_val)
        else:
            raise VaultResolutionError(
                f"AWS secret '{secret_id}' contains neither SecretString nor SecretBinary"
            )

    if key is not None:
        try:
            parsed: Any = json.loads(secret_str)
        except Exception as exc:
            raise VaultResolutionError(
                f"AWS secret '{secret_id}' is not valid JSON, cannot extract key '{key}': {exc}"
            ) from exc

        if not isinstance(parsed, dict) or key not in parsed:
            raise VaultResolutionError(f"Key '{key}' not found in AWS secret '{secret_id}'")
        return SecretString(str(parsed[key]))

    return SecretString(secret_str)


def resolve_hashicorp_vault(
    uri: str,
    *,
    vault_addr: str | None = None,
    token: str | None = None,
    timeout: float = 30.0,
) -> SecretString:
    """Resolve a secret from HashiCorp Vault HTTP API.

    Supported syntax:
      - vault://<path>#<field>
      - vault://<path>
    """
    if not uri.startswith("vault://"):
        raise VaultResolutionError(
            f"Invalid HashiCorp Vault URI format: {uri}. Expected 'vault://...'"
        )

    path_spec = uri[len("vault://") :]
    if not path_spec:
        raise VaultResolutionError("HashiCorp Vault URI missing secret path")

    if "#" in path_spec:
        path, field = path_spec.split("#", 1)
    else:
        path, field = path_spec, None

    path = path.strip("/")
    if not path:
        raise VaultResolutionError("HashiCorp Vault secret path cannot be empty")

    effective_token = token or os.environ.get("VAULT_TOKEN")
    if not effective_token:
        raise VaultResolutionError(
            "VAULT_TOKEN environment variable or token parameter is required "
            "for HashiCorp Vault resolution"
        )

    effective_addr = vault_addr or os.environ.get("VAULT_ADDR", "http://127.0.0.1:8200")
    effective_addr = effective_addr.rstrip("/")
    url = f"{effective_addr}/v1/{path}"

    headers = {"X-Vault-Token": effective_token}
    try:
        response = requests.get(url, headers=headers, timeout=timeout)
        response.raise_for_status()
    except requests.HTTPError as exc:
        code = exc.response.status_code if exc.response is not None else None
        raise VaultResolutionError(
            f"HashiCorp Vault HTTP request failed with status {code}: {exc}"
        ) from exc
    except requests.RequestException as exc:
        raise VaultResolutionError(f"HashiCorp Vault network error: {exc}") from exc

    try:
        body: Any = response.json()
    except Exception as exc:
        raise VaultResolutionError(
            f"Failed to parse HashiCorp Vault response as JSON: {exc}"
        ) from exc

    if not isinstance(body, dict):
        raise VaultResolutionError("HashiCorp Vault returned non-dict JSON response")

    data_section = body.get("data", body)
    if (
        isinstance(data_section, dict)
        and "data" in data_section
        and isinstance(data_section["data"], dict)
    ):
        kv_data = data_section["data"]
    elif isinstance(data_section, dict):
        kv_data = data_section
    else:
        kv_data = body

    if field is not None:
        if not isinstance(kv_data, dict) or field not in kv_data:
            raise VaultResolutionError(
                f"Field '{field}' not found in HashiCorp Vault secret at '{path}'"
            )
        return SecretString(str(kv_data[field]))

    if isinstance(kv_data, dict) and len(kv_data) == 1:
        single_val = next(iter(kv_data.values()))
        return SecretString(str(single_val))

    if isinstance(kv_data, str):
        return SecretString(kv_data)

    raise VaultResolutionError(
        f"HashiCorp Vault secret at '{path}' contains multiple fields; "
        "specify target field with #<field>"
    )


def resolve_secret(uri_or_name: str | SecretString) -> SecretString:
    """Resolve a secret URI, credential reference, or env var into a SecretString enclave.

    Supported schemes:
      - env:<VAR_NAME> or bare <VAR_NAME> (environment variables)
      - op://<vault>/<item>/<field> (1Password CLI)
      - arn:aws:secretsmanager:... or aws-secretsmanager:... (AWS Secrets Manager)
      - vault://<path>#<field> (HashiCorp Vault)
    """
    if isinstance(uri_or_name, SecretString):
        return uri_or_name

    if not isinstance(uri_or_name, str):
        type_name = type(uri_or_name).__name__
        raise VaultResolutionError(f"Expected str or SecretString, got {type_name}")

    if uri_or_name.startswith("op://"):
        return resolve_1password(uri_or_name)
    if uri_or_name.startswith("arn:aws:secretsmanager:") or uri_or_name.startswith(
        "aws-secretsmanager:"
    ):
        return resolve_aws_secrets(uri_or_name)
    if uri_or_name.startswith("vault://"):
        return resolve_hashicorp_vault(uri_or_name)
    if uri_or_name.startswith("env:"):
        return resolve_env(uri_or_name)

    if "://" in uri_or_name:
        scheme = uri_or_name.split("://", 1)[0]
        raise VaultResolutionError(f"Unsupported vault scheme: '{scheme}'")

    if uri_or_name in os.environ:
        return SecretString(os.environ[uri_or_name])

    raise VaultResolutionError(f"Secret '{uri_or_name}' not found in environment or vault")
