# House construction schedule

Six tasks share a crew of 4 workers (a shared, capacity-limited
resource), and some tasks can't start until others finish:

| task | duration | crew needed |
|---|---|---|
| foundation | 4 | 2 |
| framing | 6 | 3 |
| plumbing | 3 | 2 |
| electrical | 3 | 2 |
| drywall | 4 | 2 |
| painting | 2 | 1 |

Precedence: foundation before framing; framing before plumbing *and*
electrical; both plumbing and electrical before drywall; drywall before
painting. Minimize the total project length (the finish time of the
last task, painting).

## Formulation

- One `int_range` start-time variable per task.
- `cumulative(starts, durations, demands, capacity: 4)` -- Pumpkin's
  time-table-based resource constraint -- keeps total concurrent crew
  usage at or below 4 at every instant. Note `durations`/`demands` here
  are plain constants: Pumpkin 0.5.0's `cumulative` doesn't support
  variable durations or demands.
- Precedence is six `linear_geq` constraints of the shape
  `start_after - start_before >= duration_before`, e.g. framing can't
  start before `foundation_start + 4`.
- `makespan` is an auxiliary variable tied to the painting task's finish
  time via `linear_eq` (`painting_start + 2 = makespan`, `2` being
  painting's duration), and that's what gets minimized -- `optimise`
  takes a single objective *variable*, not an arbitrary expression, so
  a derived quantity like "finish time of the whole project" needs this
  one extra variable and constraint to name it.

```json
{
  "variables": [
    { "kind": "int_range", "name": "foundation_start", "min": 0, "max": 30 },
    { "kind": "int_range", "name": "framing_start", "min": 0, "max": 30 },
    { "kind": "int_range", "name": "plumbing_start", "min": 0, "max": 30 },
    { "kind": "int_range", "name": "electrical_start", "min": 0, "max": 30 },
    { "kind": "int_range", "name": "drywall_start", "min": 0, "max": 30 },
    { "kind": "int_range", "name": "painting_start", "min": 0, "max": 30 },
    { "kind": "int_range", "name": "makespan", "min": 0, "max": 32 }
  ],
  "constraints": [
    {
      "kind": "cumulative",
      "starts": [
        { "var": "foundation_start" }, { "var": "framing_start" }, { "var": "plumbing_start" },
        { "var": "electrical_start" }, { "var": "drywall_start" }, { "var": "painting_start" }
      ],
      "durations": [4, 6, 3, 3, 4, 2],
      "demands": [2, 3, 2, 2, 2, 1],
      "capacity": 4
    },
    { "kind": "linear_geq", "terms": [{ "var": "framing_start" }, { "var": "foundation_start", "scale": -1 }], "rhs": 4 },
    { "kind": "linear_geq", "terms": [{ "var": "plumbing_start" }, { "var": "framing_start", "scale": -1 }], "rhs": 6 },
    { "kind": "linear_geq", "terms": [{ "var": "electrical_start" }, { "var": "framing_start", "scale": -1 }], "rhs": 6 },
    { "kind": "linear_geq", "terms": [{ "var": "drywall_start" }, { "var": "plumbing_start", "scale": -1 }], "rhs": 3 },
    { "kind": "linear_geq", "terms": [{ "var": "drywall_start" }, { "var": "electrical_start", "scale": -1 }], "rhs": 3 },
    { "kind": "linear_geq", "terms": [{ "var": "painting_start" }, { "var": "drywall_start", "scale": -1 }], "rhs": 4 },
    { "kind": "linear_eq", "terms": [{ "var": "painting_start" }, { "var": "makespan", "scale": -1 }], "rhs": -2 }
  ],
  "solve": { "mode": "optimise", "objective": "makespan", "direction": "minimize" },
  "max_time_seconds": 15
}
```
