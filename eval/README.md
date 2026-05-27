# Evaluation

`make eval` runs the reviewer over labeled comment fixtures and reports precision, recall,
and F1 for slop detection, plus the false-positive rate on genuine-comment controls. It
makes real `claude -p` calls, so scores vary slightly between runs.

## Categories

Each `--disable` key corresponds to a type in the inline-comment-smell taxonomy used by the
open [Oztas et al. study](https://arxiv.org/html/2504.18956v1) (which also publishes the
labeled dataset). The scheme was defined for human-written comments; the definitions in the
hook are weighted toward agent output.

| Key | Taxonomy type | Definition |
|---|---|---|
| `redundant` | obvious | fully inferable from the code ("entailment from code"); a genuine *why* is kept |
| `change-narration` | irrelevant | about the edit, history, or role — not the code as it stands |
| `non-local` | non-local | references code/issues outside the immediate scope |
| `over-explained` | too-much-information / vague | exceeds the detail needed; reads like a mini design doc |
| `commented-out` | commented-out code | code left in a comment |
| `bare-todo` | task | TODO/FIXME without a tracked, concrete item |
| `apology` | "don't excuse bad code" | self-deprecating or hedging |

Not covered: `misleading` (needs code-vs-comment context), `no-comment-on-non-obvious` (the
inverse problem), and `attribution` / `beautification` (human smells agents rarely produce).

Multi-class smell classification is hard — GPT-4 reaches ~55% accuracy (34% without the
surrounding code) on this taxonomy. So the reviewer always receives the surrounding code,
the headline metric is binary slop-vs-not, and per-category recall is secondary.

## Dataset

Each fixture carries source code, the comment under review, and a gold label — either a slop
category or a "not a smell" control (genuine why-comments and public-API docs). A grounded
benchmark uses two splits:

- **Human baseline** — the public Oztas labeled set (2,211 comment–code pairs, 8 OSS
  Java/Python projects), mapped to the keys above.
- **Agent split** — Claude-Code-authored comments.

Labels use at least two annotators with reported agreement (Cohen's κ) and balanced sampling
across categories. Detection baselines for comparison: ML random forest 69%, GPT-4 55%.

## Measuring avoidance

End-to-end effect is measured by running an agent over a task suite with the hook enabled
vs. disabled and comparing slop density (slop comments per 100 LOC) in the final committed
code, alongside revision-acceptance — how often a flagged comment is fixed after the hook fires.

## Files

- `run.py` — the harness: drives `--review` over the fixtures and computes the metrics.
- `FIXTURES` in `run.py` — the labeled seed.

## References

Comment-smell taxonomy and detection (open, arXiv full-text):

- Oztas, Torun, Tüzün — *Towards Automated Detection of Inline Code Comment Smells* —
  [arxiv.org/html/2504.18956v1](https://arxiv.org/html/2504.18956v1).
- *Beyond Strict Rules: LLMs for Code Smell Detection* —
  [arxiv.org/html/2601.09873v1](https://arxiv.org/html/2601.09873v1).
- CRAIC — *Detecting Redundant Method Comments* —
  [arxiv.org/abs/1806.04616](https://arxiv.org/abs/1806.04616).

Agent-generated code quality (open, arXiv full-text):

- Zhu, Tsantalis, Rigby — *AI-Generated Smells* —
  [arxiv.org/html/2605.02741](https://arxiv.org/html/2605.02741).
- Liu et al. — *Debt Behind the AI Boom* —
  [arxiv.org/html/2603.28592v2](https://arxiv.org/html/2603.28592v2).

Comment standards (book):

- Robert C. Martin — *Clean Code: A Handbook of Agile Software Craftsmanship* (Pearson, 2008) —
  [official page](https://www.informit.com/store/clean-code-a-handbook-of-agile-software-craftsmanship-9780132350884).
