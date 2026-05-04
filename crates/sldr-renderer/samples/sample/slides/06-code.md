---
title: "A small example"
summary: "Code layout — exercises code background, syntax theme, monospace font"
tags: [sample, code]
layout: default
---

# A small example

Two functions doing the same thing. The shape of each tells you which paradigm wrote it.

```python
def score_handcrafted(state):
    s = 0
    s += material(state) * 1.0
    s += mobility(state) * 0.2
    s += king_safety(state) * 0.4
    return s

def score_learned(state, model):
    return model(encode(state))
```

The first encodes *what we know*. The second encodes *what we've measured*.
