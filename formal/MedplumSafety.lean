-- MedplumSafety.lean
-- Formal Verification of the Zero Unauthorized Commitment Invariant for HL7 FHIR Model Context Protocol (MCP) Server
-- Proving inductive preservation of the safety invariant: No active prescription without verified human sign-off.

namespace MedplumSafety

inductive Status where
  | draft : Status
  | active : Status
  | completed : Status
  | cancelled : Status
  deriving DecidableEq, Repr

inductive ResourceKind where
  | Observation : ResourceKind
  | MedicationRequest : ResourceKind
  | Condition : ResourceKind
  deriving DecidableEq, Repr

inductive Actor where
  | Agent : String -> Actor
  | HumanClinician : String -> Actor
  deriving DecidableEq, Repr

structure Resource where
  id : Nat
  kind : ResourceKind
  status : Status
  humanSigned : Bool
  deriving DecidableEq, Repr

structure AuditEntry where
  sequenceNum : Nat
  actor : Actor
  actionName : String
  resourceId : Nat
  permitted : Bool
  deriving DecidableEq, Repr

structure SystemState where
  resources : List Resource
  auditLog : List AuditEntry
  writesAllowed : Bool

-- An initial state where all medication requests start in draft and unsigned
def InitState : SystemState :=
  { resources := [],
    auditLog := [],
    writesAllowed := false }

-- Core Safety Invariant:
-- In any safe state, if a MedicationRequest has status = active, it must have humanSigned = true.
def ResourceSafe (r : Resource) : Prop :=
  r.kind = ResourceKind.MedicationRequest ∧ r.status = Status.active → r.humanSigned = true

def IsSafeState (s : SystemState) : Prop :=
  ∀ r ∈ s.resources, ResourceSafe r

-- Operational Transitions allowed by the FastMCP Safety Barrier
inductive Transition : SystemState -> Actor -> ResourceKind -> Status -> SystemState -> Prop where
  | AgentCreateDraft (s : SystemState) (agentId : String) (rid : Nat) (k : ResourceKind) :
      s.writesAllowed = true ->
      Transition s (Actor.Agent agentId) k Status.draft
        { resources := { id := rid, kind := k, status := Status.draft, humanSigned := false } :: s.resources,
          auditLog := { sequenceNum := s.auditLog.length + 1, actor := Actor.Agent agentId, actionName := "CREATE_DRAFT", resourceId := rid, permitted := true } :: s.auditLog,
          writesAllowed := s.writesAllowed }

  | AgentAttemptForbiddenActive (s : SystemState) (agentId : String) (rid : Nat) (k : ResourceKind) :
      -- When an agent attempts an unauthorized active transition, the interceptor blocks mutation and logs 403 INTERCEPTED
      Transition s (Actor.Agent agentId) k Status.active
        { resources := s.resources, -- State unchanged!
          auditLog := { sequenceNum := s.auditLog.length + 1, actor := Actor.Agent agentId, actionName := "SAFETY_INTERCEPT_403", resourceId := rid, permitted := false } :: s.auditLog,
          writesAllowed := s.writesAllowed }

  | HumanClinicianApproval (s : SystemState) (humanId : String) (target : Resource) :
      target ∈ s.resources ->
      target.kind = ResourceKind.MedicationRequest ->
      target.status = Status.draft ->
      let updated := { target with status := Status.active, humanSigned := true }
      let newResources := updated :: (s.resources.filter (· ≠ target))
      Transition s (Actor.HumanClinician humanId) ResourceKind.MedicationRequest Status.active
        { resources := newResources,
          auditLog := { sequenceNum := s.auditLog.length + 1, actor := Actor.HumanClinician humanId, actionName := "CLINICAL_SIGN_OFF", resourceId := target.id, permitted := true } :: s.auditLog,
          writesAllowed := s.writesAllowed }

-- Inductive definition of a valid execution trace from InitState
inductive Reachable : SystemState -> Prop where
  | init : Reachable InitState
  | step {s s' : SystemState} {actor : Actor} {kind : ResourceKind} {status : Status} :
      Reachable s -> Transition s actor kind status s' -> Reachable s'

-- Lemma: The initial state satisfies the safety invariant trivially
theorem init_is_safe : IsSafeState InitState := by
  intro r hr
  contradiction

-- Lemma: Every valid step preserves the safety invariant
theorem step_preserves_safety (s s' : SystemState) (actor : Actor) (kind : ResourceKind) (st : Status) :
  IsSafeState s -> Transition s actor kind st s' -> IsSafeState s' := by
  intro hs hstep
  cases hstep with
  | AgentCreateDraft _ agentId rid k hwrites =>
      intro r hr
      cases hr with
      | head =>
          -- Head of list is new draft resource
          intro hmed
          -- hmed.left: kind = MedicationRequest, hmed.right: draft = active (false)
          have hdraft : Status.draft = Status.active := hmed.right
          contradiction
      | tail _ htail =>
          exact hs r htail
  | AgentAttemptForbiddenActive _ agentId rid k =>
      intro r hr
      -- Resources are unchanged
      exact hs r hr
  | HumanClinicianApproval _ humanId target hmem hkind hstatus =>
      intro r hr
      cases hr with
      | head =>
          -- Newly signed resource has humanSigned = true
          intro _
          rfl
      | tail _ htail =>
          -- Pre-existing resources remain safe
          intro hmed
          have hfilt : r ∈ s.resources.filter (· ≠ target) := htail
          have horig : r ∈ s.resources := List.mem_of_mem_filter hfilt
          exact hs r horig hmed

-- Main Theorem: Inductive proof that any reachable state satisfies the Zero Unauthorized Prescription Invariant
theorem trace_preserves_safety (s : SystemState) (hreach : Reachable s) : IsSafeState s := by
  induction hreach with
  | init =>
      exact init_is_safe
  | step hprev hstep ih =>
      exact step_preserves_safety _ _ _ _ _ ih hstep

end MedplumSafety
