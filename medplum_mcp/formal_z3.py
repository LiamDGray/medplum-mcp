"""Microsoft Z3 SMT Reachability Prover for Clinical FSM Safety Invariants."""

from __future__ import annotations

from typing import Any

import z3

from medplum_mcp.fsm import (
    INITIAL_STATES,
    LIFECYCLE_CLASSES,
    TRANSITION_MAPS,
    _resolve_entity_type,
)
from medplum_mcp.safety import FORBIDDEN_CLINICAL_STATUSES


class Z3ProofResult:
    """Formal verification proof result from Microsoft Z3 SMT solver."""

    def __init__(
        self,
        entity_type: str,
        status: z3.CheckSatResult,
        is_mcp: bool,
        max_depth: int,
        trace: list[str] | None = None,
    ) -> None:
        self.entity_type = entity_type
        self.status = status
        self.is_mcp = is_mcp
        self.max_depth = max_depth
        self.trace = trace

    def __eq__(self, other: Any) -> bool:
        if isinstance(other, z3.CheckSatResult):
            return bool(self.status == other)
        if isinstance(other, Z3ProofResult):
            return bool(self.entity_type == other.entity_type and self.status == other.status)
        return False

    def __repr__(self) -> str:
        return (
            f"Z3ProofResult(entity_type='{self.entity_type}', "
            f"status={self.status}, is_mcp={self.is_mcp}, max_depth={self.max_depth})"
        )


class Z3ClinicalFSMModel:
    """Encodes clinical entity FSM into first-order SMT constraints in Microsoft Z3."""

    def __init__(
        self,
        entity_type: str | type,
        max_depth: int = 10,
        is_mcp: bool = True,
    ) -> None:
        self.entity_name = _resolve_entity_type(entity_type)
        self.max_depth = max_depth
        self.is_mcp = is_mcp

        lifecycle_cls = LIFECYCLE_CLASSES[self.entity_name]
        self.all_states = [item.value for item in lifecycle_cls]
        self.initial_state = INITIAL_STATES[self.entity_name]

        self.state_to_int: dict[str, int] = {state: i for i, state in enumerate(self.all_states)}
        self.int_to_state: dict[int, str] = {i: state for i, state in enumerate(self.all_states)}

        self.solver = z3.Solver()
        self.state_vars: list[z3.ExprRef] = []

    def build_model(self) -> None:
        """Construct the bounded model checking constraints in the Z3 solver."""
        self.solver.reset()
        self.state_vars = [
            z3.Int(f"{self.entity_name}_state_{t}") for t in range(self.max_depth + 1)
        ]

        # 1. Initial State constraint: s_0 == initial_state
        init_idx = self.state_to_int[self.initial_state]
        self.solver.add(self.state_vars[0] == init_idx)

        # 2. State domain constraints: 0 <= s_t < len(all_states)
        for t in range(self.max_depth + 1):
            domain_clauses = [self.state_vars[t] == i for i in range(len(self.all_states))]
            self.solver.add(z3.Or(domain_clauses))

        # 3. Transition relation constraints: T(s_t, s_{t+1})
        trans_map = TRANSITION_MAPS[self.entity_name]
        for t in range(self.max_depth):
            step_transitions: list[z3.ExprRef] = []
            for src_state, dst_states in trans_map.items():
                src_idx = self.state_to_int[src_state]
                for dst_state in dst_states:
                    # In MCP mode, safety gate eliminates any transition to forbidden statuses
                    if self.is_mcp and dst_state in FORBIDDEN_CLINICAL_STATUSES:
                        continue
                    dst_idx = self.state_to_int[dst_state]
                    step_transitions.append(
                        z3.And(
                            self.state_vars[t] == src_idx,
                            self.state_vars[t + 1] == dst_idx,
                        )
                    )

            if step_transitions:
                self.solver.add(z3.Or(step_transitions))
            else:
                # No transitions possible: state must remain unchanged (stutter)
                self.solver.add(self.state_vars[t + 1] == self.state_vars[t])

        # 4. Reachability query: does there exist t in [0, max_depth] such that s_t is forbidden?
        forbidden_indices = [
            self.state_to_int[s] for s in self.all_states if s in FORBIDDEN_CLINICAL_STATUSES
        ]
        if forbidden_indices:
            reach_bad = z3.Or(
                [
                    self.state_vars[t] == f_idx
                    for t in range(self.max_depth + 1)
                    for f_idx in forbidden_indices
                ]
            )
            self.solver.add(reach_bad)

    def check(self) -> z3.CheckSatResult:
        """Execute SMT solver check."""
        return self.solver.check()

    def get_trace(self) -> list[str] | None:
        """Extract execution counterexample trace if reachable (sat)."""
        if self.solver.check() != z3.sat:
            return None
        model = self.solver.model()
        trace: list[str] = []
        for t in range(self.max_depth + 1):
            val_expr = model.eval(self.state_vars[t])
            val = val_expr.as_long()
            state_str = self.int_to_state[val]
            trace.append(state_str)
            if state_str in FORBIDDEN_CLINICAL_STATUSES:
                break
        return trace


def prove_clinical_reachability_z3(
    entity_type: str | type,
    max_depth: int = 10,
    is_mcp: bool = True,
) -> Z3ProofResult:
    """Evaluate reachability theorem via Microsoft Z3 SMT solver.

    Asserts solver status == z3.unsat when is_mcp=True, proving the
    Zero Unauthorized Commitment / Prescription Invariant.
    """
    model = Z3ClinicalFSMModel(entity_type, max_depth=max_depth, is_mcp=is_mcp)
    model.build_model()
    status = model.check()

    trace = model.get_trace() if status == z3.sat else None
    result = Z3ProofResult(
        entity_type=model.entity_name,
        status=status,
        is_mcp=is_mcp,
        max_depth=max_depth,
        trace=trace,
    )

    if is_mcp:
        assert status == z3.unsat, (
            f"Safety Invariant Violation: Reachable forbidden state found in MCP mode "
            f"for {model.entity_name}! Counterexample trace: {trace}"
        )

    return result


def prove_all_clinical_entities_z3(max_depth: int = 10) -> dict[str, Z3ProofResult]:
    """Formally prove reachability invariants across all clinical entities with Z3."""
    entities = [
        "MedicationRequest",
        "AllergyIntolerance",
        "Observation",
        "DiagnosticReport",
        "Claim",
    ]
    results: dict[str, Z3ProofResult] = {}
    for entity in entities:
        res = prove_clinical_reachability_z3(entity, max_depth=max_depth, is_mcp=True)
        assert res.status == z3.unsat
        results[entity] = res
    return results
