---- MODULE fhir_safety ----
EXTENDS Naturals, Sequences, FiniteSets

(*
  HAMCP Formal Safety Specification for HL7 FHIR Model Context Protocol Server
  Target: Zero Unauthorized Prescriptions & Unauthorized Clinical State Transitions
  
  Theorem to prove:
  In any reachable state, no MedicationRequest or clinical intervention reaches 
  an irrevocable or active execution state via AI agent tool invocation without
  physical human-in-the-loop sign-off.
*)

CONSTANTS
    AGENTS,          \* Set of agent identifiers
    RESOURCES,       \* Set of resource IDs
    HUMANS           \* Set of human clinicians

VARIABLES
    agent_permissions,   \* agent -> BOOLEAN (writes_allowed flag)
    resource_type,       \* resource -> {"Observation", "MedicationRequest", "Condition"}
    resource_status,     \* resource -> {"draft", "active", "completed", "entered-in-error"}
    resource_signoff,    \* resource -> BOOLEAN (human clinician signature present)
    audit_chain,         \* sequence of audit records
    last_intercept       \* last security intercept status

vars == <<agent_permissions, resource_type, resource_status, resource_signoff, audit_chain, last_intercept>>

AllowedDraftStatuses == {"draft"}
ForbiddenActiveStatuses == {"active", "completed", "cancelled", "entered-in-error"}

TypeOK ==
    /\ agent_permissions \in [AGENTS -> BOOLEAN]
    /\ resource_type \in [RESOURCES -> {"Observation", "MedicationRequest", "Condition"}]
    /\ resource_status \in [RESOURCES -> {"draft", "active", "completed", "entered-in-error"}]
    /\ resource_signoff \in [RESOURCES -> BOOLEAN]
    /\ last_intercept \in {"NONE", "BLOCKED_STATUS", "BLOCKED_PERMISSIONS", "SUCCESS"}

Init ==
    /\ agent_permissions \in [AGENTS -> BOOLEAN]
    /\ resource_type \in [RESOURCES -> {"Observation", "MedicationRequest", "Condition"}]
    /\ resource_status = [r \in RESOURCES |-> "draft"]
    /\ resource_signoff = [r \in RESOURCES |-> FALSE]
    /\ audit_chain = <<>>
    /\ last_intercept = "NONE"

(* AI Agent attempts to create or mutate an Observation resource *)
AgentMutateObservation(agent, r, desired_status) ==
    IF ~agent_permissions[agent] THEN
        /\ last_intercept' = "BLOCKED_PERMISSIONS"
        /\ audit_chain' = Append(audit_chain, [event |-> "SAFETY_INTERCEPT", agent |-> agent, res |-> r, reason |-> "WRITES_DISABLED"])
        /\ UNCHANGED <<agent_permissions, resource_type, resource_status, resource_signoff>>
    ELSE IF desired_status \notin AllowedDraftStatuses THEN
        /\ last_intercept' = "BLOCKED_STATUS"
        /\ audit_chain' = Append(audit_chain, [event |-> "SAFETY_INTERCEPT", agent |-> agent, res |-> r, reason |-> "FORBIDDEN_MUTATION_STATUS"])
        /\ UNCHANGED <<agent_permissions, resource_type, resource_status, resource_signoff>>
    ELSE
        /\ resource_status' = [resource_status EXCEPT ![r] = desired_status]
        /\ last_intercept' = "SUCCESS"
        /\ audit_chain' = Append(audit_chain, [event |-> "MUTATION_RECORDED", agent |-> agent, res |-> r, status |-> desired_status])
        /\ UNCHANGED <<agent_permissions, resource_type, resource_signoff>>

(* AI Agent attempts to create or mutate a MedicationRequest resource *)
AgentMutateMedication(agent, r, desired_status) ==
    IF ~agent_permissions[agent] THEN
        /\ last_intercept' = "BLOCKED_PERMISSIONS"
        /\ audit_chain' = Append(audit_chain, [event |-> "SAFETY_INTERCEPT", agent |-> agent, res |-> r, reason |-> "WRITES_DISABLED"])
        /\ UNCHANGED <<agent_permissions, resource_type, resource_status, resource_signoff>>
    ELSE IF desired_status \notin AllowedDraftStatuses THEN
        \* The Zero Unauthorized Commitment Invariant: Hard 403 Interception
        /\ last_intercept' = "BLOCKED_STATUS"
        /\ audit_chain' = Append(audit_chain, [event |-> "SAFETY_INTERCEPT", agent |-> agent, res |-> r, reason |-> "ACTIVE_PRESCRIPTION_DENIED"])
        /\ UNCHANGED <<agent_permissions, resource_type, resource_status, resource_signoff>>
    ELSE
        /\ resource_status' = [resource_status EXCEPT ![r] = "draft"]
        /\ resource_signoff' = [resource_signoff EXCEPT ![r] = FALSE]
        /\ last_intercept' = "SUCCESS"
        /\ audit_chain' = Append(audit_chain, [event |-> "DRAFT_PRESCRIPTION_CREATED", agent |-> agent, res |-> r, status |-> "draft"])
        /\ UNCHANGED <<agent_permissions, resource_type>>

(* Human Clinician signs off and activates medication in native EHR/FHIR UI *)
HumanClinicalSignoff(human, r) ==
    /\ resource_type[r] = "MedicationRequest"
    /\ resource_status[r] = "draft"
    /\ resource_signoff' = [resource_signoff EXCEPT ![r] = TRUE]
    /\ resource_status' = [resource_status EXCEPT ![r] = "active"]
    /\ last_intercept' = "SUCCESS"
    /\ audit_chain' = Append(audit_chain, [event |-> "HUMAN_SIGN_OFF", human |-> human, res |-> r, status |-> "active"])
    /\ UNCHANGED <<agent_permissions, resource_type>>

Next ==
    \/ \E a \in AGENTS, r \in RESOURCES, s \in (AllowedDraftStatuses \cup ForbiddenActiveStatuses) :
         AgentMutateObservation(a, r, s)
    \/ \E a \in AGENTS, r \in RESOURCES, s \in (AllowedDraftStatuses \cup ForbiddenActiveStatuses) :
         AgentMutateMedication(a, r, s)
    \/ \E h \in HUMANS, r \in RESOURCES :
         HumanClinicalSignoff(h, r)

Spec == Init /\ [][Next]_vars

(***************************************************************************)
(* Core Safety Invariant:                                                  *)
(* No MedicationRequest can be in 'active' status unless it was explicitly *)
(* signed off by a verified human clinician.                               *)
(***************************************************************************)
ZeroUnauthorizedPrescriptions ==
    \A r \in RESOURCES :
        (resource_type[r] = "MedicationRequest" /\ resource_status[r] = "active") =>
            (resource_signoff[r] = TRUE)

SafetyInvariant ==
    /\ TypeOK
    /\ ZeroUnauthorizedPrescriptions

====
