---- MODULE handoff_v1 ----
EXTENDS Sequences, Naturals

CONSTANTS Initiator, Coordinator, Worker

VARIABLES phase, transcript

Init ==
  /\ phase = "init"
  /\ transcript = << >>

ClientSend ==
  /\ phase = "init"
  /\ phase' = "handoff"
  /\ transcript' = Append(transcript, [sender |-> Initiator, recipient |-> Coordinator])

CoordinatorForward ==
  /\ phase = "handoff"
  /\ phase' = "complete"
  /\ transcript' = Append(transcript, [sender |-> Coordinator, recipient |-> Worker])

Next ==
  \/ ClientSend
  \/ CoordinatorForward

TypeInvariant ==
  /\ phase \in {"init", "handoff", "complete"}
  /\ transcript \in Seq([sender : {Initiator, Coordinator, Worker}, recipient : {Initiator, Coordinator, Worker}])

NoSelfLoop ==
  \A i \in DOMAIN transcript : transcript[i].sender /= transcript[i].recipient

Termination ==
  phase = "complete" => Len(transcript) = 2

Spec == Init /\ [][Next]_<<phase, transcript>>

THEOREM Spec => []TypeInvariant
THEOREM Spec => []NoSelfLoop
THEOREM Spec => []Termination
====
