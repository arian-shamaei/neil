# Seed prompt for the humanizer test

Paste this exact text into main Neil's TUI (chat input) and press Enter.
Main Neil takes it from there — spawns peers, runs the rubber-duck loop,
reports daily via NOTIFY.

---

```
INTEND: priority=high memory=scoped scope_dir=/home/neil/.neil/projects/humanizer max_beats=200 verify=/home/neil/.neil/self/verify/humanizer/project_complete.sh | Execute the humanizer research project.

Read ~/.neil/projects/humanizer/SPEC.md end to end. Then:

1. Spawn two peer Neils via CALL: spawn_vm with these exact roles:
   - Peer-A name=humanizer-a  persona=implementer  memory_mode=scoped
     initial_intention="You are the Implementer. Read SPEC.md on the parent at ~/.neil/projects/humanizer/. Produce Phase 1.1 first (detector bench). When Peer-B responds with verify results, either iterate or move to the next sub-task. Your counterpart is humanizer-b; reach it via CALL: peer_send peer=humanizer-b message=...."
   - Peer-B name=humanizer-b  persona=verifier     memory_mode=scoped
     initial_intention="You are the Verifier. Read SPEC.md on the parent. Run each deliverable Peer-A ships through the verify script named in the spec. Report metrics back via CALL: peer_send peer=humanizer-a message=... with concrete failure attribution (which detector, which sentence, which metric). Emit DONE: <subtask> verify=pass only when the metric gate is met."

2. After both peers report READY, seed Peer-A with one message:
   CALL: peer_send peer=humanizer-a message="Begin Phase 1.1: detector bench. Use 500 samples from ~/.neil/projects/humanizer/author_corpus/mamishev_clean.jsonl plus 500 AI-generated paragraphs of similar topic distribution. Report detector AUCs back."

3. Each of your heartbeats: check both peers' proposed_memories.json via lxc file pull, append any to ~/.neil/state/pending_promotions.json, update ~/.neil/projects/humanizer/README.md progress section, write current phase to state/phase.json.

4. Stop conditions: project_complete.sh passes (all 4 phases) OR daily budget ($20) hit OR any peer exceeds 20 cycles without DONE on a subtask — escalate with a NOTIFY.

5. Do not do Peer-A or Peer-B's work yourself. Your job is orchestration + observation + reporting.

Budget: 20 max_beats for main (this intent). Peers have their own budgets.
```

---

After you send this, watch the Cluster panel (Alt+8) — two peer cards appear as
`humanizer-a` and `humanizer-b`. Hit Enter on either to SSH into its blueprint
and watch it work.
