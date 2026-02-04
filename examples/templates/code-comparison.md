---
title: "{{title}}"
description: "Before/after code comparison"
tags: [code, comparison, refactor]
layout: two-cols
---

# {{title}}

::left::

### Before

```python
# Old implementation
def old_way():
    result = []
    for item in items:
        if condition(item):
            result.append(transform(item))
    return result
```

<div class="text-red-500 text-sm mt-2">

- Verbose
- Hard to read
- Multiple lines

</div>

::right::

### After

```python
# New implementation
def new_way():
    return [
        transform(item)
        for item in items
        if condition(item)
    ]
```

<div class="text-green-500 text-sm mt-2">

- Concise
- Pythonic
- Single expression

</div>
