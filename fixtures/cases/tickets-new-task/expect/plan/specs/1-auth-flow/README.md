# 1-auth-flow

<!-- daiku:narrative -->
<!-- About 250 words: the problem, the shape of the solution, who's affected, and what's out of scope. -->
<!-- /daiku:narrative -->

<!-- daiku:generated — do not edit; run `daiku overview` -->

## Status

| Phase | Status | Note |
|---|---|---|
| intake | approved |  |
| research | skipped | known domain |
| explore | skipped | no repo in the intake |
| synthesis | skipped | fewer than two repos in scope |
| spec | approved |  |
| architecture | approved |  |
| breakdown | — |  |
| publish | — |  |

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

From [architecture.md](architecture.md).

## Work map

- [**E-0001**](tickets/E-0001.md) Accounts · draft
  - [**S-0002**](tickets/S-0002.md) Sign in · `api` · draft · R1
- [**T-0003**](tickets/T-0003.md) Write docs · `api` · draft

## Dependencies

### E-0001 · Accounts

```mermaid
flowchart LR
  E_0001["E-0001 · Accounts"]
  S_0002["S-0002 · Sign in"]
```

### No epic

```mermaid
flowchart LR
  T_0003["T-0003 · Write docs"]
```

## Repos

| Repo | draft | ready | published | Tickets |
|---|---|---|---|---|
| `api` | 2 | 0 | 0 | [S-0002](tickets/S-0002.md), [T-0003](tickets/T-0003.md) |

## Builds

| Ticket | Repo | Build |
|---|---|---|
| [S-0002](tickets/S-0002.md) | `api` | not started |
| [T-0003](tickets/T-0003.md) | `api` | not started |

## Decisions

_None yet._

## Traceability

From [functional-spec.md](functional-spec.md).

| Req | Priority | Covered by |
|---|---|---|
| R1 | must | [S-0002](tickets/S-0002.md) |

## Problems

- ! README: narrative is empty

<!-- /daiku:generated -->
