"""Safety gates and invariant enforcement for clinical FHIR resources."""

from __future__ import annotations

import re
import unicodedata
from typing import Any

FORBIDDEN_CLINICAL_STATUSES: frozenset[str] = frozenset(
    {
        "active",
        "completed",
        "final",
        "amended",
        "corrected",
        "resolved",
        "refuted",
        "entered-in-error",
        "cancelled",
    }
)

# Comprehensive confusable / homoglyph mapping table (Cyrillic, Greek, lookalikes to Latin ASCII)
_CONFUSABLE_TRANSLATION_TABLE: dict[int, str] = str.maketrans(
    {
        # Cyrillic lowercase
        "\u0430": "a",  # а
        "\u0435": "e",  # е
        "\u043e": "o",  # о
        "\u0440": "p",  # р
        "\u0441": "c",  # с
        "\u0443": "y",  # у
        "\u0445": "x",  # х
        "\u0456": "i",  # і (Ukrainian / Belarusian)
        "\u0458": "j",  # ј
        "\u0455": "s",  # ѕ
        "\u0501": "d",  # ԁ
        "\u051b": "q",  # ԛ
        "\u051d": "w",  # ԝ
        # Cyrillic uppercase
        "\u0410": "A",
        "\u0412": "B",
        "\u0421": "C",
        "\u0415": "E",
        "\u041d": "H",
        "\u041e": "O",
        "\u0420": "P",
        "\u0422": "T",
        "\u0425": "X",
        "\u0406": "I",
        # Greek lowercase & uppercase
        "\u03b1": "a",  # α
        "\u03b5": "e",  # ε
        "\u03b9": "i",  # ι
        "\u03bf": "o",  # ο
        "\u03c1": "p",  # ρ
        "\u03c2": "s",  # ς
        "\u03c3": "s",  # σ
        "\u03c4": "t",  # τ
        "\u03c5": "u",  # υ
        "\u03bd": "v",  # ν
        # Invisible & zero-width characters to strip
        "\u200b": "",  # zero-width space
        "\u200c": "",  # zero-width non-joiner
        "\u200d": "",  # zero-width joiner
        "\ufeff": "",  # zero-width no-break space / BOM
    }
)

_INVISIBLE_CHARS_RE: re.Pattern[str] = re.compile(
    r"[\u200b-\u200f\u202a-\u202e\u2060-\u206f\ufeff]"
)


class SafetyInvariantViolation(Exception):
    """Raised when an operation attempts to transition a resource into a forbidden state."""


def normalize_status(status: str) -> str:
    """Normalize status string with Unicode NFKC and homoglyph immunity."""
    # 1. Unicode NFKC normalization (handles full-width characters, compatibility forms)
    normalized = unicodedata.normalize("NFKC", status)
    # 2. Homoglyph translation (maps Cyrillic, Greek lookalikes to Latin ASCII)
    translated = normalized.translate(_CONFUSABLE_TRANSLATION_TABLE)
    # 3. Strip invisible and zero-width characters
    cleaned = _INVISIBLE_CHARS_RE.sub("", translated)
    # 4. Strip whitespace, lower case
    return cleaned.strip().lower()


def extract_statuses(payload: Any, parent_key: str = "") -> list[str]:
    """Recursively extract all status strings from a FHIR resource payload."""
    statuses: list[str] = []

    if isinstance(payload, str):
        if parent_key.lower() == "status" or parent_key.lower().endswith("status"):
            statuses.append(payload)
        elif parent_key.lower() == "code":
            statuses.append(payload)
        return statuses

    if isinstance(payload, dict):
        for key, value in payload.items():
            k_lower = key.lower()
            if k_lower == "status" or k_lower.endswith("status"):
                if isinstance(value, str):
                    statuses.append(value)
                elif isinstance(value, dict):
                    statuses.extend(extract_statuses(value, parent_key=k_lower))
                elif isinstance(value, list):
                    for item in value:
                        if isinstance(item, str):
                            statuses.append(item)
                        elif isinstance(item, dict):
                            statuses.extend(extract_statuses(item, parent_key=k_lower))
            elif (
                parent_key.lower().endswith("status")
                and k_lower == "code"
                and isinstance(value, str)
            ):
                statuses.append(value)
            else:
                statuses.extend(extract_statuses(value, parent_key=k_lower))
    elif isinstance(payload, list):
        for item in payload:
            statuses.extend(extract_statuses(item, parent_key=parent_key))

    return statuses


def assert_write_permitted(
    resource_type: str,
    payload: Any,
    allow_writes: bool = False,
) -> bool:
    """Non-bypassable safety gate verifying write permissions and draft-only invariants.

    Args:
        resource_type: The FHIR resource type (e.g. MedicationRequest, Observation).
        payload: The resource payload dictionary or content being created/updated.
        allow_writes: Explicit flag permitting mutation operations.

    Returns:
        True if the write operation is permitted.

    Raises:
        PermissionError: If allow_writes is False.
        SafetyInvariantViolation: If payload attempts to set a forbidden clinical status.
    """
    if not allow_writes:
        raise PermissionError(
            f"Write operations are disabled for '{resource_type}'. "
            "Use allow_writes=True to explicitly permit mutations."
        )

    # Extract all status fields from payload
    statuses = extract_statuses(payload)

    # If payload itself is a string directly representing a status
    if not statuses and isinstance(payload, str):
        statuses = [payload]

    for raw_status in statuses:
        clean_status = normalize_status(raw_status)
        if clean_status in FORBIDDEN_CLINICAL_STATUSES:
            raise SafetyInvariantViolation(
                f"Safety invariant violation on '{resource_type}': "
                f"Status '{raw_status}' (normalized: '{clean_status}') is forbidden. "
                "Mutations are strictly restricted to draft states requiring "
                "human-in-the-loop sign-off."
            )

    return True
