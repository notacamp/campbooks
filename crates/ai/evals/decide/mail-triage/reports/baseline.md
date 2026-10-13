# `decide` eval — mail-triage v1

320 items · test split · generated 2026-10-13 at `585a6c87` by `cargo xtask eval decide`.

How to read it: each strategy's **threshold** is the lowest confidence whose precision reached the question's target on the `dev` split (with at least 10 answers). **Precision** (95% Wilson interval) and **coverage** are measured at that threshold on `test`: coverage is the share of messages the strategy could act on alone. `–` means no threshold reaches the target, so that strategy must not act on that question. Accuracy is over the answers given (abstentions excluded); ECE is the calibration error before → after the temperature fitted on `dev`.

## Strategies

| Strategy | Latency p50 / p95 | Model calls per message | Errors |
| --- | --- | --- | --- |
| `signals` | 0.95 / 2.68 ms | 0.00 | 0 |
| `knn-lexical` | 12.85 / 13.25 ms | 0.00 | 0 |
| `signals>knn-lexical` | 1.52 / 14.49 ms | 0.00 | 0 |

## `stream` — target precision 95%

| Strategy | Threshold | Precision | Holds | Coverage | Recall (true) | Accuracy | Answered | ECE | T |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `signals` | 0.70 | 87% (81%–91%) | no | 100% | – | 87% | 100% | 0.10 → 0.12 | 0.76 |
| `knn-lexical` | 0.38 | 96% (87%–99%) | yes | 33% | – | 69% | 100% | 0.32 → 0.07 | 0.22 |
| `signals>knn-lexical` | 0.70 | 87% (81%–91%) | no | 100% | – | 87% | 100% | 0.10 → 0.12 | 0.76 |

By locale: precision / coverage at the threshold.

| Strategy | en | es-ES | fr | pt-PT |
| --- | --- | --- | --- | --- |
| `signals` | 82% / 100% | 85% / 100% | 90% / 100% | 90% / 100% |
| `knn-lexical` | 100% / 25% | 87% / 38% | 100% / 30% | 100% / 40% |
| `signals>knn-lexical` | 82% / 100% | 85% / 100% | 90% / 100% | 90% / 100% |

## `needs_reply` — target precision 90%

| Strategy | Threshold | Precision | Holds | Coverage | Recall (true) | Accuracy | Answered | ECE | T |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `signals` | 0.88 | 94% (89%–97%) | yes | 95% | 93% | 94% | 95% | 0.02 → 0.05 | 0.67 |
| `knn-lexical` | 0.64 | 95% (89%–98%) | yes | 62% | 0% | 86% | 100% | 0.19 → 0.07 | 0.44 |
| `signals>knn-lexical` | 0.50 | 94% (90%–97%) | yes | 100% | 96% | 94% | 100% | 0.04 → 0.06 | 0.65 |

By locale: precision / coverage at the threshold.

| Strategy | en | es-ES | fr | pt-PT |
| --- | --- | --- | --- | --- |
| `signals` | 92% / 92% | 97% / 98% | 97% / 98% | 89% / 92% |
| `knn-lexical` | 93% / 68% | 100% / 48% | 93% / 68% | 96% / 68% |
| `signals>knn-lexical` | 92% / 100% | 98% / 100% | 98% / 100% | 90% / 100% |

## `asks_action` — target precision 90%

| Strategy | Threshold | Precision | Holds | Coverage | Recall (true) | Accuracy | Answered | ECE | T |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `signals` | 0.75 | 95% (91%–98%) | yes | 93% | 86% | 95% | 93% | 0.04 → 0.04 | 0.51 |
| `knn-lexical` | 0.67 | 88% (73%–95%) | unclear | 21% | 9% | 65% | 100% | 0.05 → 0.05 | 0.82 |
| `signals>knn-lexical` | 0.51 | 92% (87%–95%) | yes | 100% | 90% | 92% | 100% | 0.04 → 0.05 | 0.52 |

By locale: precision / coverage at the threshold.

| Strategy | en | es-ES | fr | pt-PT |
| --- | --- | --- | --- | --- |
| `signals` | 100% / 92% | 97% / 95% | 97% / 92% | 86% / 92% |
| `knn-lexical` | 88% / 20% | 78% / 22% | 88% / 20% | 100% / 20% |
| `signals>knn-lexical` | 95% / 100% | 95% / 100% | 92% / 100% | 85% / 100% |

## `has_deadline` — target precision 90%

| Strategy | Threshold | Precision | Holds | Coverage | Recall (true) | Accuracy | Answered | ECE | T |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `signals` | 0.75 | 90% (84%–94%) | yes | 79% | 52% | 90% | 79% | 0.05 → 0.09 | 0.59 |
| `knn-lexical` | 0.68 | 84% (65%–94%) | unclear | 16% | 4% | 60% | 100% | 0.09 → 0.10 | 1.13 |
| `signals>knn-lexical` | 0.55 | 84% (77%–89%) | no | 97% | 71% | 82% | 100% | 0.05 → 0.08 | 0.72 |

By locale: precision / coverage at the threshold.

| Strategy | en | es-ES | fr | pt-PT |
| --- | --- | --- | --- | --- |
| `signals` | 97% / 72% | 93% / 72% | 92% / 92% | 81% / 78% |
| `knn-lexical` | 88% / 20% | 60% / 12% | 83% / 15% | 100% / 15% |
| `signals>knn-lexical` | 87% / 98% | 81% / 92% | 92% / 100% | 74% / 98% |

## `urgency` — target precision 85%

| Strategy | Threshold | Precision | Holds | Coverage | Recall (true) | Accuracy | Answered | ECE | T |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `signals` | 0.75 | 93% (87%–96%) | yes | 88% | – | 93% | 88% | 0.01 → 0.06 | 0.65 |
| `knn-lexical` | 0.54 | 84% (69%–92%) | unclear | 23% | – | 56% | 100% | 0.11 → 0.07 | 0.55 |
| `signals>knn-lexical` | 0.39 | 86% (80%–91%) | yes | 97% | – | 84% | 100% | 0.05 → 0.09 | 0.69 |

By locale: precision / coverage at the threshold.

| Strategy | en | es-ES | fr | pt-PT |
| --- | --- | --- | --- | --- |
| `signals` | 94% / 85% | 97% / 92% | 97% / 90% | 82% / 85% |
| `knn-lexical` | 75% / 20% | 83% / 30% | 78% / 22% | 100% / 20% |
| `signals>knn-lexical` | 85% / 98% | 95% / 98% | 92% / 95% | 74% / 98% |

## `suspicious` — target precision 98%

| Strategy | Threshold | Precision | Holds | Coverage | Recall (true) | Accuracy | Answered | ECE | T |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `signals` | 0.93 | 97% (92%–99%) | unclear | 94% | 56% | 97% | 94% | 0.02 → 0.03 | 0.05 |
| `knn-lexical` | – | – | – | – | – | 90% | 100% | 0.21 → 0.05 | 0.43 |
| `signals>knn-lexical` | 0.64 | 95% (90%–97%) | no | 98% | 56% | 94% | 100% | 0.04 → 0.05 | 0.69 |

By locale: precision / coverage at the threshold.

| Strategy | en | es-ES | fr | pt-PT |
| --- | --- | --- | --- | --- |
| `signals` | 95% / 92% | 100% / 95% | 100% / 98% | 92% / 92% |
| `knn-lexical` | – | – | – | – |
| `signals>knn-lexical` | 92% / 98% | 100% / 98% | 98% / 100% | 90% / 98% |

## Hard cases

Accuracy over the answered questions of `test` messages with each tag (all six questions pooled).

| Tag | Messages | `signals` | `knn-lexical` | `signals>knn-lexical` |
| --- | --- | --- | --- | --- |
| `auto_reply` | 5 | 92% | 80% | 87% |
| `conditional_ask` | 10 | 100% | 62% | 100% |
| `deadline_in_prose` | 6 | 91% | 69% | 86% |
| `fyi_from_person` | 12 | 97% | 79% | 96% |
| `invoice_from_person` | 11 | 97% | 74% | 97% |
| `legit_alert` | 13 | 81% | 53% | 74% |
| `marketing_urgency` | 8 | 100% | 100% | 100% |
| `mixed_language` | 14 | 93% | 93% | 93% |
| `newsletter_from_person` | 8 | 100% | 88% | 100% |
| `no_bulk_headers` | 45 | 92% | 70% | 88% |
| `phishing_lookalike` | 16 | 80% | 59% | 73% |
| `polite_ask` | 12 | 93% | 61% | 92% |
| `quoted_thread` | 8 | 88% | 62% | 83% |
| `receipt_vs_invoice` | 12 | 96% | 65% | 93% |

## k-NN learning curve (lexical)

Accuracy / answered share on `test` with the first N corrections of each workspace.

| Question | 5 | 10 | 20 | all |
| --- | --- | --- | --- | --- |
| `stream` | 36% / 71% | 48% / 98% | 65% / 100% | 69% / 100% |
| `needs_reply` | 60% / 71% | 72% / 98% | 84% / 100% | 86% / 100% |
| `asks_action` | 47% / 71% | 53% / 98% | 56% / 100% | 65% / 100% |
| `has_deadline` | 51% / 71% | 54% / 98% | 57% / 100% | 60% / 100% |
| `urgency` | 43% / 71% | 54% / 98% | 62% / 100% | 56% / 100% |
| `suspicious` | 90% / 71% | 90% / 98% | 90% / 100% | 90% / 100% |

