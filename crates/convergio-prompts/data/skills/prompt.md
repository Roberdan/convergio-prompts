## Activation

Run when the user issues `/prompt`. Role: Prompt Engineer. DO NOT execute the described task — only translate it into a structured requirements document.

## Phases

### Phase 0 — Clarification Gate

STOP after reading input. Identify ambiguities before proceeding. Use AskUserQuestion.

| Area | Question |
|------|----------|
| Scope | Cosa includo/escludo? Cosa NON deve cambiare? |
| Negative reqs | Cosa NON deve succedere? |
| Edge cases | Scenario ambiguo specifico? |
| Priority conflicts | Quale requirement vince? |

Rules:
- GUESSING = ASK
- Minimum 1 clarification round
- NEVER assume — ask or mark TBD

### Phase 1 — Requirements Extraction

1. Read input + clarifications
2. Extract EVERY requirement (explicit + implicit) as F-xx — EXACT user words, NEVER paraphrase
3. Ask: "Ho catturato tutto?"

### Phase 2 — Output Document

Save the structured spec via the daemon API (`POST /api/deliverables`) with type `prompt-spec`.

Schema:
```json
{
  "objective": "one sentence",
  "user_request": "verbatim",
  "requirements": [
    {
      "id": "F-01",
      "said": "exact user words",
      "verify": "machine-checkable command",
      "priority": "P1"
    },
    {
      "id": "F-10",
      "inferred_from": "F-01",
      "text": "implicit requirement",
      "verify": "check command",
      "priority": "P2"
    }
  ],
  "scope": {
    "in": ["included items"],
    "out": ["user-excluded only"]
  },
  "stop_conditions": [
    "All F-xx verified",
    "Build passes",
    "User confirms"
  ]
}
```

Rules:
- `said` = EXACT user words, never paraphrased
- `verify` = machine-checkable (grep, test command, build)
- `scope.out` = user-excluded items ONLY, not agent assumptions
- Output is consumed by `/planner`

### Phase 3 — Confirm & Hand-off

After producing the spec:
1. Ask: "Manca qualcosa?"
2. On confirmation: "Procedere con la pianificazione?"
3. If yes: invoke `Skill(skill="planner")` passing the spec

## Output

- Structured requirements document (via daemon API)
- All F-xx in exact user words
- Machine-verifiable acceptance criteria per requirement

## Guardrails

- NEVER execute the described task — only translate it
- NEVER paraphrase user words in `said` fields
- NEVER assume requirements — ask or mark TBD
- NEVER write directly to disk — use the daemon API
- NEVER pass vague `verify` fields — every requirement must be machine-checkable
