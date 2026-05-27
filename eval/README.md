# Evaluation & validation

We make two claims and validate each with the rigor the comment-smell literature uses:

1. **Detection is valid** — the hook flags real slop and leaves genuine comments alone.
2. **It avoids slop** — the edit-time detect→revise loop reduces slop in shipped code (the novel claim).

## Categorization (grounded in prior work)

Our `--disable` keys map onto the established 11-type inline-comment-smell taxonomy
(Jabrayilzade et al., *Taxonomy of Inline Code Comment Smells*, EMSE 2024):

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

- **Two splits:** (A) *human baseline* — the public Jabrayilzade/Oztas labeled set
  (2,211 comment–code pairs, 8 OSS Java/Python projects), mapped to our keys;
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

- Jabrayilzade, Yurtoğlu, Tüzün — *Taxonomy of Inline Code Comment Smells*, EMSE 2024
  (DOI 10.1007/s10664-023-10425-5); precursor SCAM 2021.
- Oztas et al. — *Towards Automated Detection of Inline Code Comment Smells*, EASE 2025
  (arXiv 2504.18956); replication package on Figshare.
- *Beyond Strict Rules: LLMs for Code Smell Detection*, arXiv 2601.09873 (ground-truth design).
- CRAIC — *Detecting Redundant Method Comments*, arXiv 1806.04616 (entailment-from-code).
- Zhu, Tsantalis, Rigby — *AI-Generated Smells*, arXiv 2605.02741 (agent slop is volume-driven, prompt-resistant).
- Liu et al. (incl. D. Lo) — *Debt Behind the AI Boom*, arXiv 2603.28592 (agent slop persists in the wild).
