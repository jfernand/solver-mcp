# Dinner pairing

A restaurant only allows certain meal/drink combinations (wine pairing
rules), and the guest additionally refuses white wine:

- Fish pairs with White or Rosé
- Steak pairs with Red
- Pasta pairs with Rosé or White

Meals are encoded `0=Fish, 1=Steak, 2=Pasta`; drinks `0=White, 1=Red,
2=Rosé`. Given "no white wine", the only combination left is
**Steak + Red**.

## Formulation

This is a direct use of `table`: rather than expressing the pairing
rule as a chain of comparisons, just enumerate the allowed
`(meal, drink)` tuples and let `table` restrict the two variables to
one of those rows. The "no white wine" preference is a separate
`linear_neq` layered on top -- showing that `table` constraints
compose with ordinary arithmetic constraints on the same variables,
they aren't a self-contained sub-model.

```json
{
  "variables": [
    { "kind": "int_range", "name": "meal", "min": 0, "max": 2 },
    { "kind": "int_range", "name": "drink", "min": 0, "max": 2 }
  ],
  "constraints": [
    {
      "kind": "table",
      "vars": [{ "var": "meal" }, { "var": "drink" }],
      "tuples": [
        [0, 0],
        [0, 2],
        [1, 1],
        [2, 2],
        [2, 0]
      ]
    },
    { "kind": "linear_neq", "terms": [{ "var": "drink" }], "rhs": 0 }
  ],
  "solve": { "mode": "satisfy" },
  "max_time_seconds": 5
}
```
