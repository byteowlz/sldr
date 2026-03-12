---
title: "{{title}}"
description: "System architecture diagram"
tags: [architecture, diagram, technical]
layout: center
---

# {{title}}

```
Client                      Server
+---------+                 +-------------------+
| Browser |---> Frontend -->| API Gateway       |
+---------+                 |   |           |   |
                            | Service A  Service B |
                            |   |           |   |
                            |   +---> DB <--+   |
                            +-------------------+
```

Key components:
- **Frontend**: Handles user interaction
- **API Gateway**: Routes and validates requests
- **Services**: Business logic layer
