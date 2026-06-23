# Data Protection Impact Assessment (DPIA) — Campbooks

> ⚠️ Review-ready draft, not legal advice (Art. 35). A DPIA is **likely required**
> here: Campbooks does large-scale processing of communications data and performs
> **AI profiling** of correspondents, which hits several of the CNPD / EDPB
> "likely high risk" criteria. Have counsel confirm scope and sign off.

## 1. Description of the processing
Campbooks ingests a user's entire mailbox, documents, and calendar; runs AI over
them to summarise, classify, extract structured data, and build profiles of the
user's correspondents; and offers a conversational assistant and automations.

- **Nature:** automated analysis + profiling of communications and financial documents.
- **Scope:** potentially every email a user has ever received → large volumes of **third-party** personal data (senders never interacted with Campbooks).
- **Context:** B2B SaaS, closed beta; controller in Portugal; EU hosting.
- **Data:** identity, communications content, financial identifiers (NIF/tax ID, IBAN, bank statements), calendar/attendee data, behavioural patterns, derived AI profiles.

## 2. Necessity & proportionality
- **Lawful basis:** contract (the user) + legitimate interests (third-party data inherent in the user's mailbox). A **Legitimate Interests Assessment** should be recorded for A2/A3/A5. [TODO: attach LIA.]
- **Data minimisation tension:** the contact analyzer sends up to **30 historical email bodies** per run, and document analysis sends **entire files** to the AI provider. Assess whether smaller payloads suffice.
- **Transparency:** the user is informed via the privacy policy; **third-party data subjects are not** — rely on Art. 14(5)(b) disproportionate-effort exemption, and document that reliance.

## 3. Risks to data subjects
| # | Risk | Likelihood | Severity |
|---|---|---|---|
| R1 | **International transfer** of communications + financial PII to AI providers, incl. **DeepSeek (China, no adequacy)** and a **default US (Anthropic) fallback** | Med–High | High |
| R2 | Sensitive financial PII (IBAN, bank statements, tax IDs) sent to AI for document analysis | High | High |
| R3 | **Profiling** of third-party correspondents (context summaries, behavioural patterns) without their knowledge | High | Medium |
| R4 | Personal-data breach exposing mailbox/document content (no audit log to scope access; some data unencrypted at column level) | Low–Med | High |
| R5 | Indefinite retention — data kept long after it's needed | High | Medium |
| R6 | Bug-report screenshots capturing PII-laden screens, synced to GitHub (US) | Low | Medium |

## 4. Measures to address the risks
| Risk | Mitigation | Status |
|---|---|---|
| R1 | Default to an EU/EU-adequate AI provider; **remove or gate the DeepSeek option and the hardcoded Anthropic fallback**; surface provider+region; SCCs/DPF for US providers | **Planned — task #8** |
| R2 | Evaluate EU-resident document AI; redaction/minimisation before send; explicit notice | Planned — task #8 |
| R3 | LIA + transparency; let users disable contact profiling; minimise history sent | [TODO] |
| R4 | Add audit logging; column-encrypt email bodies at rest; breach runbook | Partial (runbook ✅); audit log + encryption tracked |
| R5 | Retention windows + enforcement jobs | **Planned — task #9** (sessions already 30-day ✅) |
| R6 | Warn users what a bug report captures; make GitHub sync opt-in per workspace; strip/limit | Partial |

## 5. Residual risk & sign-off
After the planned measures (esp. tasks #8/#9 + audit logging), residual risk should
fall to **[TODO: low/medium]**. The two items that must be resolved before residual
risk is acceptable are **R1 (DeepSeek/China + default US fallback)** and **R2
(financial PII to AI)**.

- Prepared by: [TODO] · Date: 2026-06-22
- DPO/advisor review: [TODO]
- Controller sign-off: [TODO]
- Prior consultation with CNPD required if residual high risk can't be mitigated (Art. 36): [TODO assess]

Last updated: 2026-06-22.
