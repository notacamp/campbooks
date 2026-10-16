## Why
#499 (`decide-model-reads-content`, ADR 0018) removed the content rules from
the decide pass, the `signals.security_alert` rule and its platform list among
them: a decision model reads the content, and header and address facts may only
veto. The baseline `conversations` spec still requires "Security alerts are a
to-do", which no code implements. AGENTS.md: specs with no code behind them
block merge. Found while building #501 (campbooks-cloud#133).

## What Changes
- Remove the requirement "Security alerts are a to-do" from `conversations`.
  How a security alert is now judged is the decision model's job under the
  `ai-decide-facts` requirements (authentication facts only veto).
- No code change. The client keeps its `security_alert` reason string so
  decisions stored before #499 still read correctly.

## Impact
- Spec: `conversations`. No code, API, migration or UI change.
