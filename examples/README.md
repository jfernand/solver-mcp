# CSP IR examples

Each file below pairs a problem stated in prose with its formulation in
the CSP intermediate representation defined in
[`../src/csp_ir.rs`](../src/csp_ir.rs) -- variable declarations, a flat
list of constraints, and a `solve` directive.

**Status**: these are worked examples of the *schema*, written by hand
against the type definitions in `csp_ir.rs`. The interpreter that turns
this JSON into `pumpkin_solver` calls (name resolution, building
`AffineView`s, validating reification legality, dispatching `solve.mode`)
doesn't exist yet -- see that file's module docs for what's left. Treat
the JSON here as "this is what a request should look like", not as
solver-verified output; where a file states an expected answer, it was
worked out by hand, not by running the model.

| # | Example | Prose problem | Constraint kinds exercised | Solve mode |
|---|---|---|---|---|
| 01 | [SEND+MORE=MONEY](01_send_more_money.md) | classic cryptarithmetic | `linear_eq`, `all_different` | satisfy |
| 02 | [8-Queens](02_n_queens.md) | place 8 queens so none attack | `all_different` (with `Expr` offsets for diagonals) | satisfy |
| 03 | [Traveling Salesman](03_traveling_salesman.md) | cheapest tour over 4 cities | `all_different`, `linear_neq`, `element`, `linear_eq`, `linear_leq`, `reify` (via `linear_eq`) | optimise |
| 04 | [House construction schedule](04_house_construction_schedule.md) | build a house with a shared 4-person crew | `cumulative`, `linear_geq`, `linear_eq` | optimise |
| 05 | [Mini Sudoku (4x4)](05_mini_sudoku.md) | fill a 4x4 grid, rows/cols/boxes distinct | `all_different` (x12), domain-pinned clues | satisfy |
| 06 | [Dinner pairing](06_dinner_pairing_table.md) | which meal/drink combos are allowed | `table`, `linear_neq` | satisfy |
| 07 | [Knapsack](07_knapsack_optimization.md) | pick items to maximize value under a weight cap | `linear_leq`, `linear_eq` (bools used as 0/1 ints) | optimise |
| 08 | [Arithmetic puzzle](08_arithmetic_puzzle.md) | digits related by product, quotient, difference, min/max | `times`, `division`, `absolute`, `maximum`, `minimum`, `plus`, `linear_eq`, `linear_lt` | satisfy |
| 09 | [Job sequencing](09_job_sequencing.md) | order 4 jobs on one machine | `disjunctive`, `linear_leq`, `linear_gt` | satisfy |
| 10 | [Dinner party logic](10_dinner_party_logic.md) | who attends, under social rules | `clause`, `conjunction`, `implied_by`, `reify` (via `conjunction`) | enumerate |

Together these touch every `ConstraintKind` variant, both `Expr` and
`BoolRef`, both reification modes (`implied_by` and `reify`), and all
three `solve.mode` values (`satisfy`, `enumerate`, `optimise`).
