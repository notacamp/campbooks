# `decide` eval — mail-triage v1

320 items · test split · generated 2026-10-16 at `c4d63371` by `cargo xtask eval decide`.

How to read it: each strategy's **threshold** is the lowest confidence whose precision reached the question's target on the `dev` split (with at least 10 answers). **Precision** (95% Wilson interval) and **coverage** are measured at that threshold on `test`: coverage is the share of messages the strategy could act on alone. `–` means no threshold reaches the target, so that strategy must not act on that question. Accuracy is over the answers given (abstentions excluded); ECE is the calibration error before → after the temperature fitted on `dev`.

## Not run

- `knn-embed`: CAMPBOOKS_EVAL_EMBED_URL is not set
- `laya`: CAMPBOOKS_EVAL_LAYA_URL is not set
- `instruct`: CAMPBOOKS_EVAL_INSTRUCT_URL is not set
- `facts>laya`: CAMPBOOKS_EVAL_LAYA_URL is not set
- `facts>knn-lexical>laya`: CAMPBOOKS_EVAL_LAYA_URL is not set
- `facts>instruct`: CAMPBOOKS_EVAL_INSTRUCT_URL is not set
- `facts>knn-embed>instruct`: CAMPBOOKS_EVAL_EMBED_URL is not set; CAMPBOOKS_EVAL_INSTRUCT_URL is not set

## Strategies

| Strategy | Latency p50 / p95 | Model calls per message | Errors |
| --- | --- | --- | --- |
| `facts` | 0.00 / 0.00 ms | 0.00 | 0 |
| `knn-lexical` | 13.00 / 13.17 ms | 0.00 | 0 |
| `facts>knn-lexical` | 13.00 / 13.17 ms | 0.00 | 0 |
| `jev` | 293.55 / 458.87 ms | 1.00 | 0 |
| `facts>jev` | 293.55 / 458.87 ms | 1.00 | 0 |

## `stream` — target precision 95%

| Strategy | Threshold | Precision | Holds | Coverage | Recall (true) | Accuracy | Answered | ECE | T |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `facts` | – | – | – | – | – | – | 0% | – → – | – |
| `knn-lexical` | 0.38 | 96% (87%–99%) | yes | 33% | – | 69% | 100% | 0.32 → 0.07 | 0.22 |
| `facts>knn-lexical` | 0.38 | 96% (87%–99%) | yes | 33% | – | 69% | 100% | 0.32 → 0.07 | 0.22 |
| `jev` | 0.49 | 98% (95%–99%) | yes | 100% | – | 98% | 100% | 0.01 → 0.02 | 0.62 |
| `facts>jev` | 0.49 | 98% (95%–99%) | yes | 100% | – | 98% | 100% | 0.01 → 0.02 | 0.62 |

By locale: precision / coverage at the threshold.

| Strategy | en | es-ES | fr | pt-PT |
| --- | --- | --- | --- | --- |
| `facts` | – | – | – | – |
| `knn-lexical` | 100% / 25% | 87% / 38% | 100% / 30% | 100% / 40% |
| `facts>knn-lexical` | 100% / 25% | 87% / 38% | 100% / 30% | 100% / 40% |
| `jev` | 100% / 100% | 98% / 100% | 98% / 100% | 98% / 100% |
| `facts>jev` | 100% / 100% | 98% / 100% | 98% / 100% | 98% / 100% |

## `needs_reply` — target precision 90%

| Strategy | Threshold | Precision | Holds | Coverage | Recall (true) | Accuracy | Answered | ECE | T |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `facts` | 0.99 | 100% (93%–100%) | yes | 32% | 0% | 100% | 32% | 0.01 → 0.00 | 0.05 |
| `knn-lexical` | 0.64 | 95% (89%–98%) | yes | 62% | 0% | 86% | 100% | 0.19 → 0.07 | 0.44 |
| `facts>knn-lexical` | 0.64 | 95% (90%–98%) | yes | 67% | 0% | 86% | 100% | 0.10 → 0.08 | 0.82 |
| `jev` | 0.51 | 91% (86%–95%) | yes | 100% | 93% | 91% | 100% | 0.07 → 0.03 | 0.69 |
| `facts>jev` | 0.51 | 92% (87%–96%) | yes | 100% | 93% | 92% | 100% | 0.05 → 0.04 | 0.78 |

By locale: precision / coverage at the threshold.

| Strategy | en | es-ES | fr | pt-PT |
| --- | --- | --- | --- | --- |
| `facts` | 100% / 28% | 100% / 28% | 100% / 32% | 100% / 42% |
| `knn-lexical` | 93% / 68% | 100% / 48% | 93% / 68% | 96% / 68% |
| `facts>knn-lexical` | 93% / 68% | 100% / 57% | 93% / 70% | 97% / 72% |
| `jev` | 90% / 100% | 100% / 100% | 88% / 100% | 88% / 100% |
| `facts>jev` | 90% / 100% | 100% / 100% | 92% / 100% | 88% / 100% |

## `asks_action` — target precision 90%

| Strategy | Threshold | Precision | Holds | Coverage | Recall (true) | Accuracy | Answered | ECE | T |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `facts` | – | – | – | – | – | – | 0% | – → – | – |
| `knn-lexical` | 0.67 | 88% (73%–95%) | unclear | 21% | 9% | 65% | 100% | 0.05 → 0.05 | 0.82 |
| `facts>knn-lexical` | 0.67 | 88% (73%–95%) | unclear | 21% | 9% | 65% | 100% | 0.05 → 0.05 | 0.82 |
| `jev` | 0.58 | 94% (89%–97%) | yes | 94% | 100% | 92% | 100% | 0.06 → 0.04 | 0.64 |
| `facts>jev` | 0.58 | 94% (89%–97%) | yes | 94% | 100% | 92% | 100% | 0.06 → 0.04 | 0.64 |

By locale: precision / coverage at the threshold.

| Strategy | en | es-ES | fr | pt-PT |
| --- | --- | --- | --- | --- |
| `facts` | – | – | – | – |
| `knn-lexical` | 88% / 20% | 78% / 22% | 88% / 20% | 100% / 20% |
| `facts>knn-lexical` | 88% / 20% | 78% / 22% | 88% / 20% | 100% / 20% |
| `jev` | 97% / 95% | 92% / 98% | 89% / 90% | 97% / 95% |
| `facts>jev` | 97% / 95% | 92% / 98% | 89% / 90% | 97% / 95% |

## `has_deadline` — target precision 90%

| Strategy | Threshold | Precision | Holds | Coverage | Recall (true) | Accuracy | Answered | ECE | T |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `facts` | – | – | – | – | – | – | 0% | – → – | – |
| `knn-lexical` | 0.68 | 84% (65%–94%) | unclear | 16% | 4% | 60% | 100% | 0.09 → 0.10 | 1.13 |
| `facts>knn-lexical` | 0.68 | 84% (65%–94%) | unclear | 16% | 4% | 60% | 100% | 0.09 → 0.10 | 1.13 |
| `jev` | 0.51 | 97% (93%–99%) | yes | 100% | 99% | 97% | 100% | 0.09 → 0.03 | 0.51 |
| `facts>jev` | 0.51 | 97% (93%–99%) | yes | 100% | 99% | 97% | 100% | 0.09 → 0.03 | 0.51 |

By locale: precision / coverage at the threshold.

| Strategy | en | es-ES | fr | pt-PT |
| --- | --- | --- | --- | --- |
| `facts` | – | – | – | – |
| `knn-lexical` | 88% / 20% | 60% / 12% | 83% / 15% | 100% / 15% |
| `facts>knn-lexical` | 88% / 20% | 60% / 12% | 83% / 15% | 100% / 15% |
| `jev` | 100% / 100% | 95% / 100% | 98% / 100% | 95% / 100% |
| `facts>jev` | 100% / 100% | 95% / 100% | 98% / 100% | 95% / 100% |

## `urgency` — target precision 85%

| Strategy | Threshold | Precision | Holds | Coverage | Recall (true) | Accuracy | Answered | ECE | T |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `facts` | – | – | – | – | – | – | 0% | – → – | – |
| `knn-lexical` | 0.54 | 84% (69%–92%) | unclear | 23% | – | 56% | 100% | 0.11 → 0.07 | 0.55 |
| `facts>knn-lexical` | 0.54 | 84% (69%–92%) | unclear | 23% | – | 56% | 100% | 0.11 → 0.07 | 0.55 |
| `jev` | 0.73 | 81% (74%–87%) | unclear | 82% | – | 78% | 100% | 0.11 → 0.07 | 1.65 |
| `facts>jev` | 0.73 | 81% (74%–87%) | unclear | 82% | – | 78% | 100% | 0.11 → 0.07 | 1.65 |

By locale: precision / coverage at the threshold.

| Strategy | en | es-ES | fr | pt-PT |
| --- | --- | --- | --- | --- |
| `facts` | – | – | – | – |
| `knn-lexical` | 75% / 20% | 83% / 30% | 78% / 22% | 100% / 20% |
| `facts>knn-lexical` | 75% / 20% | 83% / 30% | 78% / 22% | 100% / 20% |
| `jev` | 77% / 78% | 76% / 85% | 79% / 72% | 89% / 95% |
| `facts>jev` | 77% / 78% | 76% / 85% | 79% / 72% | 89% / 95% |

## `suspicious` — target precision 98%

| Strategy | Threshold | Precision | Holds | Coverage | Recall (true) | Accuracy | Answered | ECE | T |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `facts` | – | – | – | – | – | – | 0% | – → – | – |
| `knn-lexical` | – | – | – | – | – | 90% | 100% | 0.21 → 0.05 | 0.43 |
| `facts>knn-lexical` | – | – | – | – | – | 90% | 100% | 0.21 → 0.05 | 0.43 |
| `jev` | 0.54 | 100% (98%–100%) | yes | 100% | 100% | 100% | 100% | 0.06 → 0.00 | 0.15 |
| `facts>jev` | 0.54 | 100% (98%–100%) | yes | 100% | 100% | 100% | 100% | 0.06 → 0.00 | 0.15 |

By locale: precision / coverage at the threshold.

| Strategy | en | es-ES | fr | pt-PT |
| --- | --- | --- | --- | --- |
| `facts` | – | – | – | – |
| `knn-lexical` | – | – | – | – |
| `facts>knn-lexical` | – | – | – | – |
| `jev` | 100% / 100% | 100% / 100% | 100% / 100% | 100% / 100% |
| `facts>jev` | 100% / 100% | 100% / 100% | 100% / 100% | 100% / 100% |

## Hard cases

Accuracy over the answered questions of `test` messages with each tag (all six questions pooled).

| Tag | Messages | `facts` | `knn-lexical` | `facts>knn-lexical` | `jev` | `facts>jev` |
| --- | --- | --- | --- | --- | --- | --- |
| `auto_reply` | 5 | 100% | 80% | 80% | 100% | 100% |
| `conditional_ask` | 10 | 100% | 62% | 62% | 75% | 75% |
| `deadline_in_prose` | 6 | – | 69% | 69% | 97% | 97% |
| `fyi_from_person` | 12 | – | 79% | 79% | 96% | 96% |
| `invoice_from_person` | 11 | – | 74% | 74% | 100% | 100% |
| `legit_alert` | 13 | 100% | 53% | 53% | 92% | 92% |
| `marketing_urgency` | 8 | 100% | 100% | 100% | 92% | 92% |
| `mixed_language` | 14 | 100% | 93% | 93% | 99% | 99% |
| `newsletter_from_person` | 8 | 100% | 88% | 88% | 100% | 100% |
| `no_bulk_headers` | 45 | 100% | 70% | 70% | 91% | 91% |
| `phishing_lookalike` | 16 | 100% | 59% | 59% | 76% | 76% |
| `polite_ask` | 12 | – | 61% | 61% | 90% | 90% |
| `quoted_thread` | 8 | – | 62% | 62% | 98% | 98% |
| `receipt_vs_invoice` | 12 | 100% | 65% | 65% | 99% | 99% |

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

