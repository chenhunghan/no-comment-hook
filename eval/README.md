# Evaluation & validation

We make two claims and validate each with the rigor the comment-smell literature uses:

1. **Detection is valid** — the hook flags real slop and leaves genuine comments alone.
2. **It avoids slop** — the edit-time detect→revise loop reduces slop in shipped code (the novel claim).

## Categorization (grounded in prior work)

Our `--disable` keys map onto the 11-type inline-comment-smell taxonomy as used in the
open [Oztas et al. detection study](https://arxiv.org/html/2504.18956v1) (which also
publishes the labeled dataset). The category *scheme* originated on **human-written**
comments (Jabrayilzade et al., SCAM 2021 / EMSE 2024); we adopt the categories but
**re-weight them for agent output** (see the caveat and the agent-slop references below):

| Our key | Taxonomy type | Working definition |
|---|---|---|
| `redundant` | **obvious** | comment fully inferable from the code (CRAIC's "entailment from code"); genuine *why* is kept |
| `change-narration` | **irrelevant** | about the edit/history/role, not the code as it stands |
| `non-local` | **non-local** | references code/issues outside the immediate scope |
| `over-explained` | **too-much-information** / **vague** | exceeds the detail needed; reads like a mini design doc |
| `commented-out` | **commented-out code** | code left in a comment |
| `bare-todo` | **task** | TODO/FIXME without a tracked, concrete item |
| `apology` | (Clean Code "don't excuse bad code") | self-deprecating / hedging |

Deliberately excluded: `misleading` (needs code-vs-comment context we don't have),
`no-comment-on-non-obvious` (inverse problem), `attribution` / `beautification`
(human smells agents rarely produce).

**Caveat we design around:** multi-class smell classification is hard — GPT-4 reached
~55% accuracy (34% *without* surrounding code) on this taxonomy (Oztas et al., EASE 2025).
So we (a) always feed the reviewer the surrounding code, and (b) report **binary
slop-vs-not P/R as the headline**, per-category recall as secondary.

## Ground truth (protocol)

- **Two splits:** (A) *human baseline* — the public Oztas labeled set (reuses Jabrayilzade's
  labels; 2,211 comment–code pairs, 8 OSS Java/Python projects), mapped to our keys;
  (B) *agent set* — a new corpus of **Claude-Code-authored** comments (the gap in the
  literature), captured from real sessions.
- **≥2 annotators**, report inter-annotator agreement (Oztas reached 88.69%; we report
  Cohen's κ). **Balanced** sampling across categories. Include a **"not a smell"**
  control class (genuine why-comments + public-API docs).
- **Metrics:** precision / recall / F1 (overall + per category) + false-positive rate on
  controls. Compare against baselines: ML RF 69%, GPT-4 55% (Oztas et al.).
- **Real-world analog** of Jabrayilzade's PR experiment (27% acceptance): measure
  **revision-acceptance** — how often the agent fixes a flagged comment after the wake-up.

## Avoidance (the novel claim)

A/B: run an agent over N realistic tasks with the hook ON vs OFF; measure slop per 100
LOC in the *final committed* code. Proof = significant reduction at acceptable
false-positive cost.

## Files

- `run.py` — detection harness: drives `--review` over labeled fixtures, computes P/R/F1.
- `dataset/` — labeled corpora. Today: a provisional smoke seed inline in `run.py`.
  Next: the human-baseline split (ingest Jabrayilzade/Oztas) + the Claude-Code agent split.

## References

Primary (open, arXiv full-text):

- Oztas, Torun, Tüzün — *Towards Automated Detection of Inline Code Comment Smells* —
  [arxiv.org/html/2504.18956v1](https://arxiv.org/html/2504.18956v1). The comment-smell
  taxonomy we map to, the public labeled dataset, and the detection baselines (ML 69%, GPT-4 55%).
- *Beyond Strict Rules: LLMs for Code Smell Detection* —
  [arxiv.org/html/2601.09873v1](https://arxiv.org/html/2601.09873v1). Ground-truth design.
- CRAIC — *Detecting Redundant Method Comments* —
  [arxiv.org/abs/1806.04616](https://arxiv.org/abs/1806.04616). "Entailment from code" (our `redundant`).

Agent-slop context (open, arXiv full-text):

- Zhu, Tsantalis, Rigby — *AI-Generated Smells* —
  [arxiv.org/html/2605.02741](https://arxiv.org/html/2605.02741). Agent slop is volume-driven and prompt-resistant.
- Liu et al. (incl. D. Lo) — *Debt Behind the AI Boom* —
  [arxiv.org/html/2603.28592v2](https://arxiv.org/html/2603.28592v2). Agent slop persists in the wild.

Taxonomy origin (human-written comments; gated, not on arXiv):

- Jabrayilzade, Yurtoğlu, Tüzün — *Taxonomy of Inline Code Comment Smells*, EMSE 2024 —
  [doi.org/10.1007/s10664-023-10425-5](https://doi.org/10.1007/s10664-023-10425-5) (SCAM 2021 precursor).
