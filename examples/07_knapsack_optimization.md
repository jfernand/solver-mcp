# 0/1 Knapsack

Four items, each either packed or left behind, a weight capacity of 8,
and the goal of maximizing total value:

| item | weight | value |
|---|---|---|
| A | 2 | 3 |
| B | 3 | 4 |
| C | 4 | 5 |
| D | 5 | 8 |

By hand-enumeration the optimum is `{B, D}`: weight `3+5=8` (exactly at
capacity), value `4+8=12`.

## Formulation

This is deliberately the same shape of problem `solve_assignment` /
`solve_lp` handle with a *continuous* LP relaxation -- the point here
is that with genuinely binary choices, the IR reaches for CP `bool`
variables instead, and gets exact 0/1 integrality for free (no
relaxation, no rounding risk).

One `bool` per item (`take_x`). Since a `Bool` variable's integer view
is 0/1, it can appear directly as a weighted term in an ordinary
`linear_leq` / `linear_eq` -- no separate "boolean sum" constraint kind
is needed:

- `linear_leq`: `2*take_A + 3*take_B + 4*take_C + 5*take_D <= 8` (capacity)
- `linear_eq`: ties an auxiliary `total_value` variable to
  `3*take_A + 4*take_B + 5*take_C + 8*take_D`
- `optimise` maximizes `total_value`.

```json
{
  "variables": [
    { "kind": "bool", "name": "take_A" },
    { "kind": "bool", "name": "take_B" },
    { "kind": "bool", "name": "take_C" },
    { "kind": "bool", "name": "take_D" },
    { "kind": "int_range", "name": "total_value", "min": 0, "max": 20 }
  ],
  "constraints": [
    {
      "kind": "linear_leq",
      "terms": [
        { "var": "take_A", "scale": 2 },
        { "var": "take_B", "scale": 3 },
        { "var": "take_C", "scale": 4 },
        { "var": "take_D", "scale": 5 }
      ],
      "rhs": 8
    },
    {
      "kind": "linear_eq",
      "terms": [
        { "var": "take_A", "scale": 3 },
        { "var": "take_B", "scale": 4 },
        { "var": "take_C", "scale": 5 },
        { "var": "take_D", "scale": 8 },
        { "var": "total_value", "scale": -1 }
      ],
      "rhs": 0
    }
  ],
  "solve": { "mode": "optimise", "objective": "total_value", "direction": "maximize" },
  "max_time_seconds": 10
}
```
