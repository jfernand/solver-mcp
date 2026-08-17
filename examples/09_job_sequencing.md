# Single-machine job sequencing

Four jobs run on one machine that can only do one thing at a time
(unlike the house-construction example, capacity here is inherently 1
-- there's no partial sharing):

| job | duration |
|---|---|
| 1 | 3 |
| 2 | 5 |
| 3 | 2 |
| 4 | 4 |

Job 2 can't start before time 2 (a warm-up/prep constraint), and job 4
must finish by time 10.

## Formulation

This *could* be modeled as `cumulative` with `capacity: 1`, but Pumpkin
ships a dedicated `disjunctive` (unary-resource / no-overlap)
propagator that uses edge-finding reasoning specifically tuned for the
single-resource case, rather than the general time-tabling reasoning
`cumulative` uses -- so for a genuinely unary resource, `disjunctive` is
the better fit, and this example uses it instead of falling back to
`cumulative`.

- `disjunctive(starts, durations)` -- no two job intervals may overlap.
- `linear_gt`: `start_2 > 1` (i.e. `start_2 >= 2`).
- `linear_leq`: `start_4 <= 6` (`10 - duration_4`, so it finishes by 10).

```json
{
  "variables": [
    { "kind": "int_range", "name": "start_1", "min": 0, "max": 20 },
    { "kind": "int_range", "name": "start_2", "min": 0, "max": 20 },
    { "kind": "int_range", "name": "start_3", "min": 0, "max": 20 },
    { "kind": "int_range", "name": "start_4", "min": 0, "max": 20 }
  ],
  "constraints": [
    {
      "kind": "disjunctive",
      "starts": [{ "var": "start_1" }, { "var": "start_2" }, { "var": "start_3" }, { "var": "start_4" }],
      "durations": [3, 5, 2, 4]
    },
    { "kind": "linear_gt", "terms": [{ "var": "start_2" }], "rhs": 1 },
    { "kind": "linear_leq", "terms": [{ "var": "start_4" }], "rhs": 6 }
  ],
  "solve": { "mode": "satisfy" },
  "max_time_seconds": 5
}
```
