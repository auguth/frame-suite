# Contributing

Thank you for your interest in contributing to the FRAME Suite.

This project is built on **strong abstraction discipline, semantic clarity, and composable design**.
Contributions are expected to align with these principles.

These guidelines are intentionally strict. They exist to ensure long-term correctness, maintainability, and architectural consistency.

> **Note**: This document currently serves as a condensed, unified guide covering code style, contribution practices, documentation standards, and architectural principles. As the project matures, these guidelines may be separated into more structured and dedicated documents.

## 🚀 Getting Started

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run checks and tests
5. Submit a pull request

## 📦 How to Read the Codebase

To understand the system, follow the structure in order:

* `frame/`: start here to learn the core abstractions and semantic building blocks
* `pallets/`: see how abstractions are composed into concrete runtime modules
* `runtime/`: understand how behavior is configured and assembled

This order reflects the architectural flow: **abstractions -> composition -> configuration**

## 🟢 First Contributions

If you are new to the project, start with:

* readability improvements  
* naming and comments  
* small refactors (`[cleanup]`)  
* tests (`[tests]`)  

These help you understand the system before working on deeper changes.

For contributors working with FRAME Suite pallets and abstractions, a common and effective contribution path is through `frame/frame_plugins/`.

* extend behavior by implementing new plugin models  
* follow existing trait contracts  
* keep models generic, composable, and reusable  
* contribute models you build for your own use so they are available to others  

The system is designed so that behavior is extended via plugins rather than modifying core logic, ensuring improvements benefit the broader ecosystem instead of remaining local to a single runtime.

## 🧩 Contribution Scope

### ✅ Quick Contributions

The following are encouraged and reviewed quickly:

* readability and structural clarity
* naming, spelling, and comments
* duplication removal
* error clarity and documentation
* memory efficiency (avoid unnecessary cloning)
* type usage and inference
* test coverage and invariant checks
* unused or uncovered code paths
* rewriting imperative logic into functional form
* adding new plugin models in `frame/frame_plugins/`

Use tags in PR titles:

```
[cleanup] simplify X 
[tests] add coverage for Y
```

### ⚠️ Larger Changes

Must be discussed via issues before implementation:

* logical or behavioral changes
* storage model updates
* new abstractions or features
* performance-critical refactors

## 🧠 Code Style & Design

Code must follow a **flat, explicit, and composable structure**.

### Control Flow

Execution must follow an **utmost flat, straight path**.

* prefer early returns and fail-fast behavior
* avoid deep nesting unless explicitly justified
* indentation should exist **only for syntactical constructs**:
  * `match`
  * loops
  * minimal branching

Use `if else` sparingly:
  * fast-path checks
  * single-condition logic
  * primarily for **failure handling and early exits**

Prefer **negative condition checks** when handling failure paths, as they naturally express unexpected cases.

Example:

```rust
// ❌ Avoid deep nesting
if condition {
    if other {
        process();
    }
}

// ✅ Prefer flat structure
if !condition {
    return;
}
if !other {
    return;
}
process();
```

### Pattern Preference

* prefer `match` over chained conditionals
* use `let ... else` where applicable

### Abstractions & Behavior

Logic must be expressed through **traits and semantic modules**, not free functions.

* define behavior as **minimal semantic units**
* each unit represents the lowest common denominator of behavior
* a unit may include multiple traits if under a single responsibility

Implementations must **compose units**, not bundle logic.

Free functions must not be used to express core domain behavior

* must be **private by default**
* may only be exposed via a **trait-based abstraction**
* are otherwise restricted to boundary layers only, such as:
  - UI
  - RPC interfaces
  - external bindings (e.g. C-FFI–like semantics)


### Behavioral Guarantees

All behavior must be:

* **pluggable**
* **swappable**
* **runtime-configurable**

> The runtime defines behavior, not the implementation.

## 🧱 Abstractions, Modularity & Structure

* prefer existing abstractions; do not redefine or specialize unnecessarily
* avoid duplication at all levels (types, traits, aliases, logic)
* reuse a **single declared symbol consistently** so changes propagate globally

Modules must be:

* independent
* loosely coupled
* composable

### Separation of Concerns

Clearly separate:

* low-level primitives (unchecked, composable)
* high-level logic (validated, invariant-enforcing)

### Local Reasoning

Design must allow **local reasoning**:

* no reliance on global assumptions
* guarantees enforced locally or via trait contracts

### Type System & Safety

Types must ensure correctness:

- private fields
- controlled APIs
- no direct mutation
- validated state transitions only

Public fields are only allowed when the type:

- carries **no invariants**, and
- represents a purely passive data structure

For types with invariants:

- constructors must enforce all invariants at creation time
- fields must remain private
- all mutations must preserve invariants through controlled APIs

Invariant enforcement must never rely on external usage assumptions.

### Explicitness

All behavior must be explicit:

* no hidden defaults
* no implicit assumptions
* intent must be clear at call sites

Prefer **typed dispatch** over dynamic branching.

Execution must remain:

* forward
* deterministic
* without backtracking

All public abstractions must be:

* complete
* cohesive
* meaningful

## ✨ Abstraction Ergonomics & Expressibility

* abstractions must prioritize **caller ergonomics and expressibility**

Call sites should remain:

* simple
* clean
* easy to reason about

Internal implementations may be:

* complex
* generic
* dense

but this complexity must not leak to the caller.

## 🛡️ Runtime Safety & Debug Guarantees

* **global invariants are asserted in `debug` builds only**
  * `release` builds must propagate errors instead of asserting
* no panics in runtime paths  
  * no `.unwrap()` / `.expect()` outside debug/tests (unless explicitly guarded and documented)
* failures must be **explicit, typed, and safely returned**
* all fallible operations must be handled (no unchecked unwraps)

## 📝 Readability, Naming & Comments

### Naming

* must reflect **intent and domain meaning**
* avoid ambiguity or overloaded semantics

### Comments

Comments must:

* explain **why**, not what
* document invariants, guarantees, and assumptions
* clarify non-obvious behavior

Avoid redundancy. Prefer clarity over brevity.

### File Structure

Large files must remain:

* visually structured
* categorically distinct
* internally coherent

Use consistent sectioning:

```rust

// ===============================================================================
// ```````````````````````````````` OUTER SECTION ````````````````````````````````
// ===============================================================================

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````` INNER SECTION ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

        //  -----------------------------  SUB INNER SECTION  -----------------------------  

```

Group code into:

* description
* imports
* aliases
* implementations
* helpers
* tests

Imports must be grouped and ordered consistently.
Complex types should be aliased for clarity.

## 🏗️ Architectural Principles

* start with documentation; define the model before implementation
* APIs must expose a clear **mental model**
* traits represent **semantic contracts**, not just capability
* abstract logic must remain **storage-agnostic**

Behavior must be externalized:

* injected via traits
* resolved via plugins or configuration

Prefer **lazy evaluation** over eager mutation where appropriate.

## 🧪 Testing

* test all new logic
* cover edge cases and invariants
* avoid untested paths

Tests must remain **scoped and local**:

* files test their own logic only
* cross-module behavior belongs in integration tests

Prefer readability:

* avoid heavy generics
* use resolved type aliases
* prioritize clarity over abstraction

## 🛠 Tooling

Before submitting:

```
cargo fmt
cargo clippy --all-targets --all-features
cargo test
```

All checks must pass.

## 🧭 Contribution Expectations & Guidelines

The points below restate core principles from this document in a form that is directly applicable during contribution and review.

### Pull Request Expectations

A good PR should:

* be **small and focused** (single concern)
* include a clear description of:
  * what changed
  * why it changed
  * how it aligns with existing abstractions
* reference related issues (if applicable)
* include tags (`[cleanup]`, `[tests]`)
* include tests where relevant
* avoid mixing refactors with behavioral changes
* ensure compilation and tests pass

PRs that are easier to review are more likely to be merged quickly.

### Plugin Contributions

When contributing a plugin:

* implement existing trait contracts; do not introduce new abstractions unless discussed
* keep the plugin:
  * generic
  * composable
  * independent of specific runtime assumptions
* avoid embedding business logic that cannot be reused

If possible, include:

* a minimal usage example
* tests that demonstrate behavior in isolation

### When to Open an Issue First

Open an issue before starting work if your change:

* introduces new traits or abstractions
* modifies storage structure or data flow
* changes runtime behavior or invariants
* affects performance-critical paths

A short proposal (problem + approach) is sufficient.

### Invariants

An invariant is any condition that must always hold true for a type or module.

* invariants must be enforced:
  * at construction time, or
  * through controlled APIs
* they must not rely on external usage assumptions
* violations must be prevented by design, not convention

Document invariants where they are defined.

### Control Flow Boundaries

As a guideline:

* avoid nesting beyond 2 levels where possible
* prefer early returns to reduce indentation
* deeper nesting is acceptable only when it improves clarity

Clarity takes precedence over strict rule enforcement.

### Error Handling

* define explicit error types per module or domain
* avoid generic or opaque error types
* errors should convey:
  * cause
  * context (when necessary)

Prefer returning `Result` with domain-specific error enums.

### Non-Goals

This project does not prioritize:

* convenience over clarity
* implicit behavior or hidden defaults
* tightly coupled or specialized solutions

Trade-offs should favor composability and explicit design.

### 🧭 Maintainer Perspective

This project emphasizes long-term correctness, composability, and clarity in design.

Because of this, reviews may focus more on architectural alignment than just whether something works. Suggestions or changes in PRs are meant to ensure consistency across the system and to keep the codebase easy to reason about over time.

If you're unsure about a direction-especially for larger changes-opening an issue first is always a good way to align early.

Contributions of all sizes are appreciated, and even small improvements (naming, structure, tests) help strengthen the system.

## 📌 Final Notes

This project prioritizes:

* correctness
* modularity
* composability
* efficiency

These guidelines are intentionally rigorous.

They reduce ambiguity, enforce clarity, and ensure the system remains predictable and scalable over time.

We recognize that this level of discipline can feel demanding.
It is necessary to preserve the integrity of the system.

## 📜 License

By contributing, you agree that your contributions will be licensed under MPL-2.0.
