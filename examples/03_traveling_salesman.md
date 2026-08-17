# Traveling Salesman (4 cities)

Four cities `0, 1, 2, 3` with a symmetric distance matrix:

|      | 0  | 1  | 2  | 3  |
|------|----|----|----|----|
| **0**| -  | 10 | 15 | 20 |
| **1**| 10 | -  | 35 | 25 |
| **2**| 15 | 35 | -  | 30 |
| **3**| 20 | 25 | 30 | -  |

Find the cheapest round trip that visits every city exactly once and
returns to the start. By hand-enumeration the optimum is the tour
`0 -> 1 -> 3 -> 2 -> 0` (or its reverse), length `10+25+30+15 = 80`.

## Formulation

Pumpkin has no built-in `circuit` global constraint, so this is the
standard decomposition: a **successor** representation plus
**Miller-Tucker-Zemlin (MTZ)** subtour elimination. This is the example
that most needs reification, so it's worth walking through:

- `next_i`: the city visited immediately after city `i`. `all_different`
  over `next_0..next_3` plus `next_i != i` makes it a permutation with
  no self-loops -- but that alone still allows disjoint sub-cycles
  (e.g. `0->1->0` and `2->3->2`), not just one big tour.
- **Distances as constants**: `element` (`array[index] = rhs`) needs its
  `array` entries to be variables, not literal numbers, so each distance
  is modeled as an `int_range` variable pinned to a single value
  (`min == max`) -- the same "pin a variable to a constant" trick used
  for Sudoku's clues. `zero` fills the unused diagonal slot in each
  city's distance row (a city is never its own successor, so that slot
  is never actually read).
- **Cost per city**: `element(array: dist_row_i, index: next_i, rhs: cost_i)`
  looks up the distance from `i` to wherever `next_i` points.
  `total = cost_0 + cost_1 + cost_2 + cost_3` via one `linear_eq`, and
  that's the `optimise` objective.
- **Subtour elimination**: introduce a boolean `x_i_j` for each ordered
  pair among cities `{1, 2, 3}` (city `0` is the fixed "depot" and
  doesn't need one), meaning "the tour goes directly from `i` to `j`".
  Each is tied to the successor variables with a *full* reification:
  `x_i_j <-> (next_i == j)`, i.e. `reify` on a `linear_eq` -- this is
  exactly why `linear_eq` supports `reify` and not just `implied_by`,
  since the model needs to reason in both directions (knowing `x_i_j`
  tells you `next_i`, and vice versa).
  Then the standard MTZ inequality, posted unconditionally (no
  reification needed here -- `x_i_j` being 0/1 already linearizes the
  implication):
  `u_i - u_j + 3*x_i_j <= 2` for every `i != j` in `{1, 2, 3}`
  (`3 = n-1`, `2 = n-2` for `n = 4` cities). If `x_i_j = 1` this forces
  `u_j >= u_i + 1`, so `u` increases strictly along any cycle confined
  to `{1, 2, 3}` -- which is impossible for a genuine cycle. The only
  topology left satisfying both `all_different` and this is one
  Hamiltonian circuit through all 4 cities.

```json
{
  "variables": [
    { "kind": "int_range", "name": "next_0", "min": 0, "max": 3 },
    { "kind": "int_range", "name": "next_1", "min": 0, "max": 3 },
    { "kind": "int_range", "name": "next_2", "min": 0, "max": 3 },
    { "kind": "int_range", "name": "next_3", "min": 0, "max": 3 },

    { "kind": "int_range", "name": "cost_0", "min": 0, "max": 35 },
    { "kind": "int_range", "name": "cost_1", "min": 0, "max": 35 },
    { "kind": "int_range", "name": "cost_2", "min": 0, "max": 35 },
    { "kind": "int_range", "name": "cost_3", "min": 0, "max": 35 },
    { "kind": "int_range", "name": "total", "min": 0, "max": 140 },

    { "kind": "int_range", "name": "u_1", "min": 1, "max": 3 },
    { "kind": "int_range", "name": "u_2", "min": 1, "max": 3 },
    { "kind": "int_range", "name": "u_3", "min": 1, "max": 3 },

    { "kind": "bool", "name": "x_1_2" },
    { "kind": "bool", "name": "x_1_3" },
    { "kind": "bool", "name": "x_2_1" },
    { "kind": "bool", "name": "x_2_3" },
    { "kind": "bool", "name": "x_3_1" },
    { "kind": "bool", "name": "x_3_2" },

    { "kind": "int_range", "name": "zero", "min": 0, "max": 0 },
    { "kind": "int_range", "name": "dist_0_1", "min": 10, "max": 10 },
    { "kind": "int_range", "name": "dist_0_2", "min": 15, "max": 15 },
    { "kind": "int_range", "name": "dist_0_3", "min": 20, "max": 20 },
    { "kind": "int_range", "name": "dist_1_0", "min": 10, "max": 10 },
    { "kind": "int_range", "name": "dist_1_2", "min": 35, "max": 35 },
    { "kind": "int_range", "name": "dist_1_3", "min": 25, "max": 25 },
    { "kind": "int_range", "name": "dist_2_0", "min": 15, "max": 15 },
    { "kind": "int_range", "name": "dist_2_1", "min": 35, "max": 35 },
    { "kind": "int_range", "name": "dist_2_3", "min": 30, "max": 30 },
    { "kind": "int_range", "name": "dist_3_0", "min": 20, "max": 20 },
    { "kind": "int_range", "name": "dist_3_1", "min": 25, "max": 25 },
    { "kind": "int_range", "name": "dist_3_2", "min": 30, "max": 30 }
  ],
  "constraints": [
    { "kind": "all_different", "vars": [{ "var": "next_0" }, { "var": "next_1" }, { "var": "next_2" }, { "var": "next_3" }] },
    { "kind": "linear_neq", "terms": [{ "var": "next_0" }], "rhs": 0 },
    { "kind": "linear_neq", "terms": [{ "var": "next_1" }], "rhs": 1 },
    { "kind": "linear_neq", "terms": [{ "var": "next_2" }], "rhs": 2 },
    { "kind": "linear_neq", "terms": [{ "var": "next_3" }], "rhs": 3 },

    { "kind": "element", "array": [{ "var": "zero" }, { "var": "dist_0_1" }, { "var": "dist_0_2" }, { "var": "dist_0_3" }], "index": { "var": "next_0" }, "rhs": { "var": "cost_0" } },
    { "kind": "element", "array": [{ "var": "dist_1_0" }, { "var": "zero" }, { "var": "dist_1_2" }, { "var": "dist_1_3" }], "index": { "var": "next_1" }, "rhs": { "var": "cost_1" } },
    { "kind": "element", "array": [{ "var": "dist_2_0" }, { "var": "dist_2_1" }, { "var": "zero" }, { "var": "dist_2_3" }], "index": { "var": "next_2" }, "rhs": { "var": "cost_2" } },
    { "kind": "element", "array": [{ "var": "dist_3_0" }, { "var": "dist_3_1" }, { "var": "dist_3_2" }, { "var": "zero" }], "index": { "var": "next_3" }, "rhs": { "var": "cost_3" } },

    {
      "kind": "linear_eq",
      "terms": [{ "var": "cost_0" }, { "var": "cost_1" }, { "var": "cost_2" }, { "var": "cost_3" }, { "var": "total", "scale": -1 }],
      "rhs": 0
    },

    { "kind": "linear_eq", "terms": [{ "var": "next_1" }], "rhs": 2, "reification": { "mode": "reify", "literal": { "var": "x_1_2" } } },
    { "kind": "linear_eq", "terms": [{ "var": "next_1" }], "rhs": 3, "reification": { "mode": "reify", "literal": { "var": "x_1_3" } } },
    { "kind": "linear_eq", "terms": [{ "var": "next_2" }], "rhs": 1, "reification": { "mode": "reify", "literal": { "var": "x_2_1" } } },
    { "kind": "linear_eq", "terms": [{ "var": "next_2" }], "rhs": 3, "reification": { "mode": "reify", "literal": { "var": "x_2_3" } } },
    { "kind": "linear_eq", "terms": [{ "var": "next_3" }], "rhs": 1, "reification": { "mode": "reify", "literal": { "var": "x_3_1" } } },
    { "kind": "linear_eq", "terms": [{ "var": "next_3" }], "rhs": 2, "reification": { "mode": "reify", "literal": { "var": "x_3_2" } } },

    { "kind": "linear_leq", "terms": [{ "var": "u_1" }, { "var": "u_2", "scale": -1 }, { "var": "x_1_2", "scale": 3 }], "rhs": 2 },
    { "kind": "linear_leq", "terms": [{ "var": "u_1" }, { "var": "u_3", "scale": -1 }, { "var": "x_1_3", "scale": 3 }], "rhs": 2 },
    { "kind": "linear_leq", "terms": [{ "var": "u_2" }, { "var": "u_1", "scale": -1 }, { "var": "x_2_1", "scale": 3 }], "rhs": 2 },
    { "kind": "linear_leq", "terms": [{ "var": "u_2" }, { "var": "u_3", "scale": -1 }, { "var": "x_2_3", "scale": 3 }], "rhs": 2 },
    { "kind": "linear_leq", "terms": [{ "var": "u_3" }, { "var": "u_1", "scale": -1 }, { "var": "x_3_1", "scale": 3 }], "rhs": 2 },
    { "kind": "linear_leq", "terms": [{ "var": "u_3" }, { "var": "u_2", "scale": -1 }, { "var": "x_3_2", "scale": 3 }], "rhs": 2 }
  ],
  "solve": { "mode": "optimise", "objective": "total", "direction": "minimize" },
  "max_time_seconds": 15
}
```
