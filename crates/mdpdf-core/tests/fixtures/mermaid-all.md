# Mermaid diagram families

## Flowchart

```mermaid {caption="Flowchart"}
flowchart LR
  A[Start] --> B{Decision} --> C[Finish]
```

## Sequence

```mermaid {caption="Sequence"}
sequenceDiagram
  Alice->>Bob: Hello
  Bob-->>Alice: Hi
```

## Class

```mermaid {caption="Class"}
classDiagram
  Animal <|-- Duck
  Animal : +String name
```

## State

```mermaid {caption="State"}
stateDiagram-v2
  [*] --> Ready
  Ready --> Done
  Done --> [*]
```

## Entity relationship

```mermaid {caption="Entity relationship"}
erDiagram
  CUSTOMER ||--o{ ORDER : places
```

## Gantt

```mermaid {caption="Gantt"}
gantt
  dateFormat YYYY-MM-DD
  section Delivery
  Design :a1, 2026-10-01, 3d
  Build :after a1, 5d
```

## Pie

```mermaid {caption="Pie"}
pie title Tasks
  "Done" : 60
  "Pending" : 40
```

## Journey

```mermaid {caption="Journey"}
journey
  title User journey
  section Work
    Edit: 5: User
    Export: 5: User
```

## Git graph

```mermaid {caption="Git graph"}
gitGraph
  commit
  branch feature
  commit
  checkout main
  merge feature
```

## Mindmap

```mermaid {caption="Mindmap"}
mindmap
  root((Document))
    Markdown
    Diagrams
    PDF
```

## Timeline

```mermaid {caption="Timeline"}
timeline
  title Delivery
  2026 : Core : Renderer
  2027 : Polish
```

## Quadrant

```mermaid {caption="Quadrant"}
quadrantChart
  title Priorities
  x-axis Low Effort --> High Effort
  y-axis Low Impact --> High Impact
  quadrant-1 Plan
  quadrant-2 Do
  quadrant-3 Skip
  quadrant-4 Review
  Core: [0.3, 0.8]
```

## Sankey

```mermaid {caption="Sankey"}
sankey-beta
  Source,Process,10
  Process,Output,8
  Process,Waste,2
```

## XY chart

```mermaid {caption="XY chart"}
xychart-beta
  title "Exports"
  x-axis [Jan, Feb, Mar]
  y-axis "Count" 0 --> 100
  bar [20, 45, 80]
```

## Block

```mermaid {caption="Block"}
block-beta
  columns 2
  A["Editor"] B["Preview"]
```

## Architecture

```mermaid {caption="Architecture"}
architecture-beta
  group app(cloud)[App]
  service server(server)[Server] in app
  service db(database)[Store] in app
  server:R -- L:db
```

## Requirement

```mermaid {caption="Requirement"}
requirementDiagram
  requirement parity {
    id: 1
    text: preview equals PDF
    risk: low
    verifymethod: test
  }
  element renderer {
    type: software
  }
  renderer - satisfies -> parity
```

## C4 context

```mermaid {caption="C4 context"}
C4Context
  Person(user, "User", "Writes Markdown")
  System(app, "mdpdf", "Prints PDFs")
  Rel(user, app, "Uses")
```

## Packet

```mermaid {caption="Packet"}
packet-beta
  0-15: "Source port"
  16-31: "Destination port"
```

## Kanban

```mermaid {caption="Kanban"}
kanban
  todo[Todo]
    task[Write Markdown]
  done[Done]
    export[Export PDF]
```

## Radar

```mermaid {caption="Radar"}
radar-beta
  title Skills
  axis r["Read"], w["Write"], p["Print"]
  curve a["App"]{4, 5, 5}
```

## Treemap

```mermaid {caption="Treemap"}
treemap-beta
  "Documents"
    "Reports": 40
    "Notes": 60
```

## Info

```mermaid {caption="Info"}
info
```

## Swimlane

```mermaid {caption="Swimlane"}
swimlane-beta LR
  subgraph Writer
    edit[Edit]
  end
  subgraph Renderer
    print[Print]
  end
  edit --> print
```

## Event modeling

```mermaid {caption="Event modeling"}
eventmodeling
tf 01 ui Editor
tf 02 cmd Export
tf 03 evt Printed
```

## Tree view

```mermaid {caption="Tree view"}
treeView-beta
├── assets/
└── report.md
```

## Ishikawa

```mermaid {caption="Ishikawa"}
ishikawa-beta
    Clipped labels
    Typography
        Font unavailable
    Layout
        Diagram too wide
```

## Railroad primitives

```mermaid {caption="Railroad primitives"}
railroad-beta
export = sequence(terminal("print"), nonterminal("document")) ;
```

## Railroad EBNF

```mermaid {caption="Railroad EBNF"}
railroad-ebnf-beta
export = "print", "document" ;
```

## Railroad ABNF

```mermaid {caption="Railroad ABNF"}
railroad-abnf-beta
export = "print" SP "document" ;
```

## Railroad PEG

```mermaid {caption="Railroad PEG"}
railroad-peg-beta
Export <- "print" "document" ;
```

## Venn

```mermaid {caption="Venn"}
venn-beta
  set Preview
  set PDF
  union Preview,PDF["Shared renderer"]
```

## Wardley

```mermaid {caption="Wardley"}
wardley-beta
title Document pipeline
anchor Reader [0.9, 0.5]
component Renderer [0.6, 0.7]
Reader -> Renderer
```

## Cynefin

```mermaid {caption="Cynefin"}
cynefin-beta
  clear
    "Apply preset"
  complicated
    "Tune typography"
  complex
    "Explore layout"
  chaotic
    "Recover browser"
```
