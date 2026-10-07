# EXP-095 finite packet correction

Reproduce from recorded baseline using project Python with:

```sh
PYTHONPATH=. .venv/bin/python docs/experiments/evidence/EXP-095/replay.py.txt baseline /tmp/exp095-baseline.json
PYTHONPATH=. .venv/bin/python docs/experiments/evidence/EXP-095/replay.py.txt candidate /tmp/exp095-candidate.json
PYTHONPATH=. .venv/bin/python docs/experiments/evidence/EXP-094/citation-codec-tests.py.txt
```

The worker uses the real experimental packet projection, label encoder and task
codebook with three public synthetic captures. It changes only the encoder function
in process memory for candidate replay; historical files remain byte-identical.
Raw model packets and expected complete conversions are retained in each JSON.
No network, model calls, stage clock, permit, private packet or quality analysis.

This supports the one-line correction as a finite measurement prerequisite. The
patch is inert proposed successor material. It does not modify the old run or
constitute an admitted successor. Actual frozen-capture handoff, retained assignment
validation, fresh source closure/review/admission and all study gates remain necessary.
