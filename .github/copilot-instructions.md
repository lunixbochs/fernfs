# Copilot PR Review Agent

**Trust these instructions.** Search only if something here is missing or provably wrong.  
**Language:** write PR review comments in **English**. Keep **code comments & Rustdoc in English**.

## One True Validation Sequence (run in order, stop on first failure)
1) `cargo fmt --all -- --check`  
2) `cargo clippy --workspace --all-features -- -D warnings`  
3) `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`  
4) `cargo test --workspace --all-features --doc`  
5) `cargo test --workspace --all-features`

CI mirrors this sequence and blocks merges on any warning or failure.

---

# Global PR Hygiene

- Write review comments in **English**; code comments & Rustdoc in **English**
- Split big PRs into logical parts; provide an informative **English** description
- Always run (in order) before approval: fmt → clippy (-D warnings) → doc (deny warnings) → doctests → tests
- Ensure files end with a final newline (EOF newline rule)
- No `FIXME` in PRs; use `TODO: #XXXX`

---

# Rust Style & Code Quality Guidelines

## Import Organization
- **Import order (groups, blank line between; alphabetical inside):**
  1) `std::…` → 2) external crates → 3) internal modules of current crate (`self::…`, local `mod …;`)
  → 4) workspace crates (`crate::…`/org) → 5) `super::…`
- **Module layout:** public types → public traits → private aliases/types/methods → tests
- **Avoid repeating module names in type names** (use namespaces via modules)

## Documentation Standards
- **Every public item MUST have Rustdoc** (English only) with `# Parameters` and `# Returns` sections (if applicable)
- **Module-level Rustdoc** explaining purpose & key abstractions
- **Test documentation:** brief Rustdoc for test functions explaining what is being tested
- **Examples in docs:** include usage examples for public APIs when helpful

## Code Readability & Structure
- **Prevent "code drifting right"** - extract conditions & helpers, prefer simple `if/match`
- **Function length:** keep functions under 50 lines, extract complex logic into helper functions
- **Variable naming:** use descriptive names, avoid abbreviations except for common patterns (e.g., `id`, `url`)
- **Type annotations:** prefer explicit types for public APIs and complex expressions

## SOLID Principles

### Single Responsibility Principle (SRP)
- **Each function/struct/trait should have ONE reason to change**
- **Functions should do ONE thing well**
- **Avoid "god objects"** - split large structs into smaller, focused ones
- **Extract complex logic** into separate functions or modules

### Open/Closed Principle (OCP)
- **Open for extension, closed for modification**
- **Use traits for extensibility** - prefer trait objects over concrete types in public APIs
- **Design APIs that can be extended** without modifying existing code
- **Use the Strategy pattern** for different implementations

### Liskov Substitution Principle (LSP)
- **Trait implementations must be substitutable** for their trait objects
- **Don't violate trait contracts** - implementations should behave as expected
- **Use trait bounds** to ensure type safety and substitutability

### Interface Segregation Principle (ISP)
- **Keep traits focused and small** - don't force clients to depend on methods they don't use
- **Split large traits** into smaller, more specific ones
- **Use marker traits** for compile-time guarantees

### Dependency Inversion Principle (DIP)
- **Depend on abstractions, not concretions**
- **Use trait objects or generics** instead of concrete types
- **Inject dependencies** rather than creating them inside functions
- **Use the Factory pattern** for complex object creation

## Performance & Memory Management
- **Avoid unnecessary allocations** - use references and slices when possible
- **Prefer `&str` over `String`** for function parameters when ownership isn't needed
- **Use `Cow<'a, T>`** for conditional cloning
- **Consider `Box<dyn Trait>` vs generics** based on usage patterns
- **Profile before optimizing** - focus on algorithmic improvements first

## Common Anti-patterns to Avoid
- **Don't use `clone()`** to work around borrow checker - fix the design instead
- **Avoid `Vec<Box<dyn Trait>>`** when `Vec<T>` with trait bounds would work
- **Don't return `Result<T, Box<dyn Error>>`** - use specific error types
- **Avoid `as` casts** - use proper type conversions or redesign
- **Don't ignore `#[must_use]`** warnings

---

# Error Handling, Logging, Time

- **No `expect(...)`.** `unwrap()` only under a *proven* invariant with nearby English comment. Prefer typed errors and `?`
- Convert error types explicitly with `map_err` (avoid blanket `From` for error typing)
- **Don't lose internal errors in logs**, even if user-visible result is "cancelled/success"
- **tracing** logs are structured (use `target` and fields), no ALL-CAPS; don't invent special "error" fields
- **JSON:** Prefer `{}` for empty root; avoid `null` as empty object (use only when semantically required)
- **Time in JSON:** use ISO-8601 (`chrono::DateTime<Utc>`). Measure durations with `Instant`; record start/end with `SystemTime`

---

# Concurrency: Channels, Tasks, Threads

- **Async channels:** use **unbounded**; send via `send_blocking`/`try_send`; enforce unbounded with `debug_assert_eq!(sender.capacity(), None)`. Don't `unwrap()` on channel ops (can be closed)
- **System threads:** always keep `JoinHandle` and **join** on drop/finish to avoid leaks/flaky tests
- **Guards:** use RAII guards to send final signals/cleanup in `Drop` (e.g., send `Close` when all work is done)
- **Start tasks exactly once** by consuming the runner; avoid multiple launches/races

---

# Testing Policy

- New component ⇒ new unit tests; also test supported trait implementations
- Tests must be **fast** and **short**; avoid multithreaded tests and randomness
- Async tests: use timeout and **single-threaded** local runtime
- Test only **public** API; keep tests independent (may run in parallel)
- Use `tempfile`, no hardcoded paths/ports. Avoid comparing huge slices in `assert_eq!`
- Don't add extra derives (`Debug/Display/Eq/Ord`) just "for tests"
- **Property-based testing:** use `proptest` for complex logic and edge case discovery
- **Test error conditions:** ensure error paths are covered, not just happy paths
- **Mock external dependencies** using traits for better test isolation
- **Integration tests:** test module boundaries and cross-component interactions

---

# Commits & PR Process

- Subject ≤ 72 chars with component prefix; **Github issue in footer** (`Fixes/Resolves/Implements/Closes #XXXX`). First commit in PR must reference Github
- Prefer squashing locally before PR; force-push is allowed to fix history if branch is already pushed
- PR description in **English**, review discussion should be in **English**
- No `FIXME`; if future work is needed, leave `TODO: #XXXX`
- End every file with a newline

---

# CI & Tooling

- Toolchain pinned via `rust-toolchain.toml` (`stable` + `clippy`, `rustfmt`)
- CI must run (and block on) the same local sequence:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-features -- -D warnings`
  - `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`
  - `cargo test --workspace --all-features --doc`
  - `cargo test --workspace --all-features`
- Treat doc & clippy **warnings as errors**. Ensure Rustdoc & doctests pass

---

# Code Review Checklist
- [ ] Imports are properly ordered and grouped
- [ ] All public items have Rustdoc
- [ ] Functions follow SRP and are reasonably sized
- [ ] Types are used effectively (no unnecessary `Box`, `Arc`, etc.)
- [ ] Code follows Rust idioms and conventions
- [ ] Performance implications are considered
- [ ] Error handling is appropriate and safe
- [ ] No `unwrap()`/`expect()` without justification
- [ ] Tests cover the new functionality
- [ ] No `unsafe` code without thorough justification
