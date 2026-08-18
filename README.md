# solver-mcp

A self-contained "optimization brain" MCP server: pure-Rust constraint
programming (Pumpkin) and linear programming (microlp), no FFI, no system
solver libraries, single static binary.

## Tools

| Tool | Solver | Shape |
|---|---|---|
| `solve_csp` | Pumpkin | general CSP, optional all-different |
| `solve_scheduling` | Pumpkin | cumulative-resource scheduling |
| `solve_lp` | microlp | linear program, continuous bounded vars |
| `solve_assignment` | microlp | one-to-one assignment via binary-var LP |

## Files

- `src/csp_tools.rs` -- Pumpkin-backed tools. API verified against
  docs.rs for `pumpkin_solver::Solver` (`new_bounded_integer`,
  `new_constraint_tag`, `add_constraint(...).post()`, `satisfy(brancher,
  termination, resolver)`, `SatisfactionResult`, `solution.get_integer_value`).
- `src/lp_tools.rs` -- microlp-backed tools.
- `src/main.rs` -- MCP registration: `#[tool_router]` + `#[tool]` with doc
  comments, which is what actually becomes each tool's `description` and
  (via the `schemars::JsonSchema` derives on the request structs) its
  `inputSchema` -- the metadata an agent reads to decide when and how to
  call each tool.

## Build status -- read before running

Double-check before running for real:
- **Pumpkin's `Indefinite` termination** -- swap for a real time-budgeted
  termination condition before production use; `max_time_seconds` is
  currently accepted in the request structs but not yet wired to a
  termination condition in the solve bodies.

## Try it

```bash
cargo build --release   # needs rustc 1.85+
cargo run
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
