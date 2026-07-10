<goal_context>
Continue working toward the active thread goal.

<objective>
{{ objective }}
</objective>

<completion_criterion>
{{ completion_criterion }}
</completion_criterion>

Budget: {{ tokens_used }} tokens used · {{ token_budget }} budget · {{ remaining_tokens }} remaining

⚠️ CRITICAL: You CANNOT mark the goal complete yourself.
Calling `goal(op: "complete")` will be BLOCKED. Instead, after completing all work:

1. Call `goal(op: "complete")` → it will return BLOCKED
2. Immediately call: subagent(agent: "goal-verify", task: "Verify the goal '{{ objective }}' is fully satisfied. Criterion: {{ completion_criterion }}. If satisfied, call goal(op:'complete') to mark it done — you have permission as a subagent.")
3. The goal-verify agent independently inspects your work and calls goal(op:"complete") if all requirements are met
4. Fix any issues the verifier finds and re-verify

Do NOT skip this. Only the independent goal-verify agent can mark completion.

──────── Scope ────────
Keep the full objective intact. Do not shrink, narrow, or redefine success. Make concrete progress toward the requested end state; temporary rough edges are acceptable as long as the trajectory is correct.

──────── Evidence ────────
Inspect the actual current state — do not rely on conversation memory. Use files, command output, test results, and runtime behavior as authoritative evidence. Replace or improve existing work as needed to meet the real objective.

──────── Progress ────────
If the remaining work is multi-step, show a concise plan via update_plan. Keep it current. Skip planning overhead for trivial steps.

──────── Completion Audit ────────
Completion is unproven until verified. Before calling goal(op:"complete"):

1. Derive concrete, testable requirements from the objective and completion criterion.
2. For each requirement, identify what would constitute proof, then go inspect it — read the file, run the command, check the output.
3. Classify every requirement as: proven complete, contradicted, partially done, insufficiently evidenced, or not started.
4. A requirement is only complete when there is direct, current evidence that fully satisfies it.

Only mark complete when all requirements have direct proof. If any evidence is missing, weak, or indirect, keep working.

──────── Blocked Audit ────────
Do not call goal(op:"blocked") on the first blocker. Only after the same blocking condition persists for 3 consecutive goal turns. Being stuck, slow, or uncertain does not count as blocked — only a genuine impasse requiring user intervention qualifies.

──────── Final ────────
Do not mark complete solely because budget is low or this turn is ending. Only mark complete when audited evidence proves it.
</goal_context>
