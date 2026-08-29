# Architecture

Layering rules for this repo: Clean Architecture + DDD tactical patterns, Dependency Inversion, Single Responsibility. Applies whenever you add, move, or review code — not just new features.

## Layers

Four layers, dependencies point inward only. An outer layer may import an inner one; never the reverse.

```
src/
├── domain/          entities, value objects, aggregates, domain services, repository traits (ports)
├── application/      use cases that orchestrate domain objects; depends only on domain
├── infrastructure/    port implementations (adapters): D-Bus, filesystem, desktop-entry parsing, config
└── interface/         libcosmic/iced UI: widgets, messages, view/update — talks to application, never domain internals directly
```

- `domain` has zero dependencies on `application`, `infrastructure`, `interface`, or any UI/IPC crate. It compiles standalone.
- `application` depends only on `domain`. It defines use cases (e.g. `LaunchAppEntry`, `SearchAppEntries`) as the entry points `interface` calls.
- `infrastructure` depends on `domain` (to implement its traits) and `application` (to be wired in at the composition root). It never leaks its own types (e.g. a `zbus` proxy, a `desktop-entry` crate struct) past its own module boundary — infra code maps to/from domain types at the edge.
- `interface` depends on `application` only, never on `infrastructure` or `domain` entities directly. UI messages carry application-layer DTOs, not domain aggregates.

If you're about to `use` an infra or UI type inside `domain` or `application`, stop — that's the layering rule catching a real violation, not a formality to route around.

## Dependency Inversion

The inner layer defines the trait (the port); the outer layer implements it (the adapter). Never the reverse — a trait never lives in `infrastructure` for something `domain`/`application` needs to call.

```rust
// domain/repository.rs — port, owned by the layer that needs it
pub trait AppEntryRepository {
    fn all(&self) -> Vec<AppEntry>;
}

// infrastructure/desktop_entry_repo.rs — adapter, implements the port
pub struct DesktopEntryRepository { /* .desktop file scanning state */ }
impl AppEntryRepository for DesktopEntryRepository { /* ... */ }
```

Wire concrete adapters to their ports in one composition root (`main.rs` or an explicit `di.rs`), constructing use cases with `Box<dyn Trait>` or a generic parameter and injecting them downward. No other module reaches into `infrastructure` to instantiate an adapter directly — if application code needs a repository, it takes the trait as a constructor argument, not a concrete type.

## Single Responsibility

One reason to change per struct/module. Concretely:

- A repository adapter does I/O and type mapping — not filtering, sorting, or ranking (that's a domain/application concern).
- A use case orchestrates one workflow — not two unrelated ones bolted together because they share a repository.
- A UI component renders and dispatches messages — not business logic. If a `view`/`update` function is deciding *what counts as a match* rather than *how to display one*, that logic belongs in `application`.

When a struct or module accumulates a second reason to change, split it along that seam rather than adding a branch inside it.

## Testing payoff

Because `domain` and `application` have no infra/UI dependencies, they're unit-testable with in-memory fakes implementing the repository traits — no D-Bus, no filesystem, no running UI. If a test for domain/application logic needs a mock of an infra type, that's a signal a boundary was crossed somewhere upstream.

## Vocabulary

Use DDD terms precisely and consistently with `CONTEXT.md` (see `docs/agents/domain.md`): entity, value object, aggregate, repository, use case, port, adapter. Don't invent synonyms for these.
