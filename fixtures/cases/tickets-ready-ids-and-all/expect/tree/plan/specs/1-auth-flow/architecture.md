# Architecture — Auth flow

## Diagrams

### Context

```mermaid
flowchart LR
  user --> app
```

### Layers

```mermaid
flowchart TB
  cli --> core
```

### Flow: sign in

```mermaid
sequenceDiagram
  user->>app: sign in
```
