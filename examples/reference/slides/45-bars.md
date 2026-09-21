---
title: Bars
subtitle: a ```bars fence draws a labelled bar list
layout: framed-flow
footer: "© sldr"
source: "Data blocks | Reference"
tags: [reference, guide]
---
::left::

1. **Ask**
   One line per bar.
2. [x] **Score**
   `label | value unit`
3. **Pick**
   The bold label is the highlight.

::right::

Percent values scale against 100, plain numbers against the largest value. Units pass through to the value column.

```bars
# comment lines and blank lines are skipped
**blue** | 93 %
clear | 68 %
not | 26 %
… | 8 %
```

Fill, track and highlight follow `--sldr-diagram-*`, so a flavor restyles every bar list at once.

<!-- notes: The chain on the left is the vertical form of framed-flow: with column markers the left column holds the chain, the right column the commentary, and lists on the right stay ordinary bullets. -->
