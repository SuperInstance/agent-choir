# agent-choir

**Scaling from quartet (4 agents) to choir (100+ agents).**

At quartet scale, every agent hears every other — communication is direct,
coordination is trivial, and the group fits in a single conversation. At choir
scale, you need *sections*: groups of agents that share a role, a range, and
a blend target. This crate provides the data structures and algorithms for
organising, scoring, and directing large ensembles of cooperative agents.

## Core Concepts

### Voice Sections (Soprano / Alto / Tenor / Bass)

Every agent has a preferred voice part and a set of capabilities. The four
classical voice parts — Soprano (highest), Alto, Tenor, Bass (lowest) — map
naturally to agent roles in a distributed system: high-level planners,
intermediate coordinators, task executors, and infrastructure workers.

A `VoiceSection` groups N agents under one part. Each section tracks its
members, computes aggregate skill levels, and can be assessed independently.

### Blend Score

A `BlendScore` measures how well agents within a section blend with each other.
Blend is a composite of:

- **Skill uniformity** (40%) — how similar the skill levels are. A section
  where everyone is at 0.7 blends better than one with a 0.3 and a 1.0.
- **Part affinity** (40%) — fraction of singers whose *preferred* part matches
  the section they're in. Misplaced singers hurt blend.
- **Versatility** (20%) — average versatility helps sections adapt.

The score ranges from 0.0 (poor blend) to 1.0 (perfect blend).

### Choir Balance

`ChoirBalance` measures section-to-section balance. A choir is balanced when
sections have comparable size and comparable average skill. The balance uses
a coefficient-of-variation approach: perfect balance scores 1.0, and increasing
divergence drives the score toward 0.0.

### Score Allocation

`allocate_parts()` assigns singers to voice parts based on their capabilities.
The algorithm works in two passes:

1. **Preferred assignment** — each singer goes to their preferred section.
2. **Overflow redistribution** — if a section exceeds its target size, the
   weakest singers are redistributed to the most underfull section.

Each allocation includes a confidence score: `skill` for preferred-part
assignments, or `skill × versatility` for cross-part assignments.

### Choir Director

The `ChoirDirector` coordinates sections without micromanaging. Given target
sizes and thresholds, the director:

- Issues `Directive`s to each section: `Grow`, `Shrink`, `Rehearse`, or `Hold`.
- Runs full assessments combining balance, blend, and directives.
- Scales from quartets (1 per section) to choirs of 100+ agents.

## Usage

```rust
use agent_choir::*;

// Create a director targeting 25 per section (100-voice choir).
let mut targets = std::collections::HashMap::new();
targets.insert(VoicePart::Soprano, 25);
targets.insert(VoicePart::Alto, 25);
targets.insert(VoicePart::Tenor, 25);
targets.insert(VoicePart::Bass, 25);

let mut director = ChoirDirector::new(targets, 0.5, 0.5);

// Add singers (in practice, from your agent registry).
for i in 0..100 {
    let part = VoicePart::all()[i % 4];
    director.add_singer(Singer {
        id: format!("agent-{i:03}"),
        preferred_part: part,
        versatility: 0.5,
        skill: 0.6 + (i as f64 * 0.004),
    });
}

// Full assessment.
let assessment = director.full_assessment();
println!("Balance: {:.2}", assessment.balance.overall);
for (part, blend) in &assessment.blends {
    println!("{part:?} blend: {:.2}", blend.score);
}
```

## Design Philosophy

This crate treats multi-agent coordination as a musical metaphor because the
mapping is surprisingly precise:

| Musical Concept     | Systems Analogue                    |
|---------------------|-------------------------------------|
| Voice part          | Agent role / capability tier        |
| Section             | Agent group with shared role        |
| Blend               | Homogeneity of performance          |
| Balance             | Load distribution across groups     |
| Director            | Orchestrator / scheduler            |
| Rehearsal           | Tuning / calibration phase          |

The key insight: **at quartet scale, coordination is O(n²) and every agent
knows every other. At choir scale, coordination must be O(n) via sections.**
The director doesn't talk to individual singers — it talks to sections, and
sections manage their own internal dynamics.

## License

MIT
