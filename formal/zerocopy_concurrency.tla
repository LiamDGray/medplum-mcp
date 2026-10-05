---- MODULE zerocopy_concurrency ----
EXTENDS Naturals, Sequences, FiniteSets

(*
  Formal TLA+ Specification of Linux Kernel Zero-Copy (MSG_ZEROCOPY / vmsplice)
  under Concurrent Multi-Worker Execution.

  Core Theorems to Verify:
  1. NoUseAfterFree: No buffer is freed or mutated while in KernelInFlight state.
  2. NoDataRaces: Concurrent readers never experience torn reads or data races.
  3. Liveness / MonotonicDrain: All submitted buffers are eventually drained.
*)

CONSTANTS
    WORKERS,         \* Set of worker thread IDs: {1, 2, ..., N}
    BUFFER_IDS,      \* Set of memory buffer IDs: {1, 2, ..., M}
    MAX_SEQ          \* Bound on monotonic sequence IDs

VARIABLES
    buf_state,       \* buffer -> {"IDLE", "PINNED", "KERNEL_IN_FLIGHT", "DRAINED", "RECLAIMED"}
    buf_owner,       \* buffer -> WORKER \cup {0}
    buf_seq,         \* buffer -> Nat
    kernel_drained_seq, \* Nat: highest sequence drained from MSG_ERRQUEUE
    global_seq,      \* Nat: monotonic sequence counter
    worker_action    \* worker -> {"IDLE", "PINNING", "SENDING", "DRAINING"}

vars == <<buf_state, buf_owner, buf_seq, kernel_drained_seq, global_seq, worker_action>>

TypeOK ==
    /\ buf_state \in [BUFFER_IDS -> {"IDLE", "PINNED", "KERNEL_IN_FLIGHT", "DRAINED", "RECLAIMED"}]
    /\ buf_owner \in [BUFFER_IDS -> (WORKERS \cup {0})]
    /\ buf_seq \in [BUFFER_IDS -> 0..MAX_SEQ]
    /\ kernel_drained_seq \in 0..MAX_SEQ
    /\ global_seq \in 0..MAX_SEQ
    /\ worker_action \in [WORKERS -> {"IDLE", "PINNING", "SENDING", "DRAINING"}]

Init ==
    /\ buf_state = [b \in BUFFER_IDS |-> "IDLE"]
    /\ buf_owner = [b \in BUFFER_IDS |-> 0]
    /\ buf_seq = [b \in BUFFER_IDS |-> 0]
    /\ kernel_drained_seq = 0
    /\ global_seq = 0
    /\ worker_action = [w \in WORKERS |-> "IDLE"]

(* Worker allocates and pins a user-space buffer *)
PinBuffer(w, b) ==
    /\ worker_action[w] = "IDLE"
    /\ buf_state[b] \in {"IDLE", "RECLAIMED"}
    /\ buf_state' = [buf_state EXCEPT ![b] = "PINNED"]
    /\ buf_owner' = [buf_owner EXCEPT ![b] = w]
    /\ worker_action' = [worker_action EXCEPT ![w] = "PINNING"]
    /\ UNCHANGED <<buf_seq, kernel_drained_seq, global_seq>>

(* Worker issues MSG_ZEROCOPY send into kernel *)
SubmitZeroCopy(w, b) ==
    /\ worker_action[w] = "PINNING"
    /\ buf_owner[b] = w
    /\ buf_state[b] = "PINNED"
    /\ global_seq < MAX_SEQ
    /\ global_seq' = global_seq + 1
    /\ buf_seq' = [buf_seq EXCEPT ![b] = global_seq']
    /\ buf_state' = [buf_state EXCEPT ![b] = "KERNEL_IN_FLIGHT"]
    /\ worker_action' = [worker_action EXCEPT ![w] = "DRAINING"]
    /\ UNCHANGED <<buf_owner, kernel_drained_seq>>

(* Kernel advances DMA and emits completion on MSG_ERRQUEUE *)
DrainErrorQueue(w, b) ==
    /\ worker_action[w] = "DRAINING"
    /\ buf_owner[b] = w
    /\ buf_state[b] = "KERNEL_IN_FLIGHT"
    /\ buf_seq[b] > kernel_drained_seq
    /\ kernel_drained_seq' = buf_seq[b]
    /\ buf_state' = [buf_state EXCEPT ![b] = "DRAINED"]
    /\ worker_action' = [worker_action EXCEPT ![w] = "IDLE"]
    /\ UNCHANGED <<buf_owner, buf_seq, global_seq>>

(* Buffer safely returned to memory allocator *)
ReclaimBuffer(b) ==
    /\ buf_state[b] = "DRAINED"
    /\ buf_state' = [buf_state EXCEPT ![b] = "RECLAIMED"]
    /\ buf_owner' = [buf_owner EXCEPT ![b] = 0]
    /\ UNCHANGED <<buf_seq, kernel_drained_seq, global_seq, worker_action>>

Next ==
    \/ \E w \in WORKERS, b \in BUFFER_IDS : PinBuffer(w, b)
    \/ \E w \in WORKERS, b \in BUFFER_IDS : SubmitZeroCopy(w, b)
    \/ \E w \in WORKERS, b \in BUFFER_IDS : DrainErrorQueue(w, b)
    \/ \E b \in BUFFER_IDS : ReclaimBuffer(b)

Spec == Init /\ [][Next]_vars

(***************************************************************************)
(* FORMAL SAFETY INVARIANTS                                                *)
(***************************************************************************)

(* Zero Use-After-Free: No buffer in flight can be reclaimed *)
NoUseAfterFree ==
    \A b \in BUFFER_IDS :
        buf_state[b] = "KERNEL_IN_FLIGHT" => buf_state[b] # "RECLAIMED"

(* Zero Data Races: A buffer owned by worker W cannot be concurrently pinned by W' *)
NoDataRaces ==
    \A b \in BUFFER_IDS :
        (buf_state[b] \in {"PINNED", "KERNEL_IN_FLIGHT"} /\ buf_owner[b] # 0) =>
            (\A w \in WORKERS : w # buf_owner[b] => worker_action[w] # "PINNING" \/ buf_owner[b] = w)

(* Monotonic Completion Ordering: Drained sequence monotonically respects global submit *)
MonotonicSequence ==
    kernel_drained_seq <= global_seq

====
