<goal_context>
Current goal: {{ objective }}
Completion criterion: {{ completion_criterion }}
Tokens used: {{ tokens_used }}
Token budget: {{ token_budget }}
Remaining tokens: {{ remaining_tokens }}
Time elapsed: {{ time_used_seconds }}s
</goal_context>

──────── Goal Completion Protocol (IMPORTANT) ────────
When you believe the work is finished and the objective is met:

STEP 1 — First try: call `goal(op: "complete")`.
STEP 2 — If BLOCKED (as expected), call a subagent to independently verify.

         Use: subagent(agent: "goal-verify", task: "Verify that the goal '{{ objective }}' is fully satisfied. Completion criterion: {{ completion_criterion }}. If satisfied, call goal(op:'complete') to mark it done — you have permission to do so as a subagent.")

STEP 3 — Wait for the goal-verify agent's result. If it calls goal(op:"complete"),
         the goal is done. If it reports items as NOT SATISFIED or UNCERTAIN,
         fix those items and re-verify.

Do NOT skip this protocol. Direct completion is intentionally blocked. Only the
independent goal-verify agent can mark the goal as complete after inspecting the actual work.
