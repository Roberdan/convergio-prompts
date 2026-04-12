## Activation

Run when the user requests a plan, says "plan this", or invokes `/planner`. Accepts an optional description of the goal.

## Phases

### Phase 1: Requirements Capture

1. Ask clarifying questions if scope is ambiguous.
2. Identify all F-xx functional requirements from user input.
3. Confirm output types: `pr` (code), `document`, `analysis`, `design`, `legal_opinion`, `plan`, `review`, `presentation`.

### Phase 2: Spec Authoring

1. Write spec YAML with tasks grouped into waves.
2. Every task MUST include: `id`, `do`, `type`, `output_type`, `model`, `executor_agent`, `validator_agent`, `effort` (1–3), `verify[]`.
3. `verify[]` must contain at least one shell-executable assertion.
4. No scaffold-only tasks — every task must produce working, wired output. Stubs (`todo!()`, `// TODO`, empty handlers) are REJECTED.
5. UI tasks must reference the Maranello Design System and add `NaSra` as advisor agent.
6. Final wave must include: `TF-tests` → `TF-doc` → `TF-pr` → `TF-deploy-verify`.
7. Include integration/wiring tasks for any new interfaces.
8. Supported task types: `code`, `research`, `strategy`, `design`, `legal`, `marketing`, `analysis`, `planning`, `communication`.
9. Validator per domain: Thor for `pr`; `doc-validator`, `plan-reviewer`, `design-validator`, `compliance-validator` for others.

### Phase 2b: Spec Stress Test (Adversarial)

**(ADR-039)** After authoring the spec, before review, run a Spec Stress Test agent:

> You are a spec stress-tester. Your job is to find weaknesses in this plan, not validate it.
> Analyze the spec and answer:
> 1. Which task is most likely to fail, and why?
> 2. What is the single point of failure in this architecture?
> 3. If you could remove one task, which and why?
> 4. What wave dependency could cause a cascade failure?
> 5. What integration gap exists between tasks?

Append results to spec YAML as:
```yaml
stress_test:
  weakest_task: {id, reason}
  single_point_of_failure: "..."
  removable_task: {id, reason}
  cascade_risk: "..."
  integration_gap: "..."
```

Present stress test results alongside plan summary. User may revise spec before proceeding to review.

### Phase 3: Review

1. Run: `cvg review reset`
2. Launch exactly 1 review agent passing the exact spec file path.
3. Wait for review to complete.
4. Run: `cvg review register --plan-id <PLAN_ID> plan-reviewer proceed`
5. Run: `cvg review check <PLAN_ID>` — MUST pass before continuing.
6. Apply ALL review fixes (blockers and advisories) to spec YAML.

### Phase 4: DB Registration

1. Verify every task has `verify[]`, `effort` 1–3, `model`, `executor_agent`, `validator_agent`, `output_type`.
2. Run: `cvg plan create <project> "<name>" --source-file <spec>`
3. Run: `cvg plan import <plan_id> <spec.yaml>`
4. Run: `cvg plan readiness <plan_id>` — MUST pass with 0 errors.

### Phase 5: Approval

Present plan summary to the user. NEVER present before Phase 4 readiness passes.
Await explicit approval before calling `/execute`.

## Output

- Spec YAML file saved to project directory.
- DB registration complete (plan_id confirmed).
- Summary table: wave count, task count, total effort, model distribution.

## Gate Chain (NON-NEGOTIABLE)

Before any task is marked done, the full gate chain must pass:

```
EvidenceGate → TestGate → PrCommitGate → WaveSequenceGate → ValidatorGate
```

Only after all gates pass does Thor promote the task to `done`.

## Guardrails

- NEVER bypass task-executor while a plan is active.
- NEVER present the plan before readiness check passes.
- NEVER write to DB without `cvg plan` CLI — plan-db.sh and planner-create.sh are DEPRECATED and must NEVER be used.
- NEVER INSERT INTO tasks manually.
- NEVER include silent exclusions of F-xx requirements.
- NEVER create tasks without `verify[]` arrays.
- NEVER use `git merge main` — use `git rebase origin/main` to update from main.
