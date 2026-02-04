---
title: "{{title}}"
description: "System architecture diagram"
tags: [architecture, diagram, technical]
layout: center
---

# {{title}}

```mermaid {scale: 0.8}
graph TB
    subgraph Client
        A[Browser] --> B[Frontend]
    end
    
    subgraph Server
        C[API Gateway] --> D[Service A]
        C --> E[Service B]
        D --> F[(Database)]
        E --> F
    end
    
    B --> C
```

<div class="text-sm opacity-70 mt-4">

Key components:
- **Frontend**: Handles user interaction
- **API Gateway**: Routes and validates requests
- **Services**: Business logic layer

</div>
