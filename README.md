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

I scaffolded and attempted to compile this in the sandbox and hit a real
wall worth knowing about:

- The sandbox's `rustc` (installed via `apt`) is **1.75.0** (Dec 2023).
- `pumpkin-solver`'s own dependency tree (via `clap` -> `clap_derive`, and
  separately via `rand` -> `chacha20` -> `cpufeatures`) now requires the
  **edition2024** Cargo feature, which needs **rustc 1.85+**.
- The sandbox can't reach `static.rust-lang.org` / `rustup.rs` to fetch a
  newer toolchain (outside its network allowlist), and Ubuntu's apt repos
  (including backports) top out at 1.75.

So: **this did not compile in the sandbox**, and I was not able to verify
it end-to-end. It should compile cleanly with a current stable Rust
(`rustup update` to 1.85+) on your own machine -- `pumpkin-solver`'s own
published MSRV is far lower (1.72.1), the wall is purely in *transitive*
dependency floors that have crept up since.

Also double-check before running for real:
- **`rmcp` version/API** -- I used a plausible `tool_router`/`tool`/
  `serve_stdio` shape based on the common Rust MCP SDK pattern, but did not
  verify current method names against docs.rs for this crate specifically.
  Check `cargo add rmcp` and its current examples.
- **Pumpkin's `Indefinite` termination** -- swap for a real time-budgeted
  termination condition before production use; `max_time_seconds` is
  currently accepted in the request structs but not yet wired to a
  termination condition in the solve bodies.

## Try it

```bash
cargo build --release   # needs rustc 1.85+
cargo run
```
