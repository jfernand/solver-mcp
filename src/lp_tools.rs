//! Linear-programming tools backed by microlp (a pure-Rust LP/MILP solver,
//! fork of the archived `minilp`).
//!
//! These tools own anything cleanly linear: resource allocation, blending,
//! cost minimization, and assignment problems modeled with binary variables.
//! For combinatorial/discrete problems (all-different, scheduling) see
//! `csp_tools.rs`.

use microlp::{ComparisonOp, OptimizationDirection, Problem};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, schemars::JsonSchema)]
pub struct LpConstraint {
    /// Coefficient for each variable, in the same order as the request's
    /// `objective` array, on this constraint's left-hand side.
    pub coeffs: Vec<f64>,
    /// One of "<=", ">=", "==".
    pub op: String,
    /// Right-hand side value of the constraint.
    pub rhs: f64,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct LpRequest {
    /// Linear objective coefficients, one per decision variable, to minimize.
    /// To maximize, negate these coefficients and negate `objective_value`
    /// in the response.
    pub objective: Vec<f64>,
    /// Linear (in)equality constraints applied to the variables.
    pub constraints: Vec<LpConstraint>,
    /// (lower_bound, upper_bound) for each variable, in the same order as
    /// `objective`. Use a large number like 1e9 to approximate "unbounded".
    pub var_bounds: Vec<(f64, f64)>,
}

#[derive(Serialize)]
pub struct LpResponse {
    /// One of "OPTIMAL" or "INFEASIBLE".
    pub status: String,
    /// Present only when status is "OPTIMAL": value of each variable, in
    /// the order given in the request.
    pub values: Option<Vec<f64>>,
    pub objective_value: Option<f64>,
}

fn parse_op(op: &str) -> ComparisonOp {
    match op {
        "<=" => ComparisonOp::Le,
        ">=" => ComparisonOp::Ge,
        _ => ComparisonOp::Eq,
    }
}

/// Solves a linear program: minimizes a linear objective subject to linear
/// (in)equality constraints over bounded continuous variables. Use for
/// resource allocation, blending, and cost-minimization problems where the
/// relationships between variables are all linear.
pub fn solve_lp(req: LpRequest) -> LpResponse {
    let mut problem = Problem::new(OptimizationDirection::Minimize);

    let vars: Vec<_> = req
        .objective
        .iter()
        .zip(&req.var_bounds)
        .map(|(&coeff, &(lo, hi))| problem.add_var(coeff, (lo, hi)))
        .collect();

    for c in &req.constraints {
        let op = parse_op(&c.op);
        let terms: Vec<_> = vars
            .iter()
            .zip(&c.coeffs)
            .map(|(&v, &co)| (v, co))
            .collect();
        problem.add_constraint(terms, op, c.rhs);
    }

    match problem.solve() {
        Ok(microlp::SolveOutcome::Solution(solution)) => LpResponse {
            status: "OPTIMAL".into(),
            objective_value: Some(solution.objective()),
            values: Some(
                vars.iter()
                    .map(|&v| solution[v])
                    .collect(),
            ),
        },
        _ => LpResponse {
            status: "INFEASIBLE".into(),
            values: None,
            objective_value: None,
        },
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AssignmentRequest {
    /// Square cost matrix: cost_matrix[i][j] is the cost of assigning
    /// worker/agent i to task j.
    pub cost_matrix: Vec<Vec<f64>>,
    /// If true, maximizes total value instead of minimizing total cost.
    #[serde(default)]
    pub maximize: bool,
}

#[derive(Serialize)]
pub struct AssignmentResponse {
    pub status: String,
    /// (agent_index, task_index) pairs for the optimal one-to-one assignment.
    pub assignment: Option<Vec<(usize, usize)>>,
    pub total_cost: Option<f64>,
}

/// Solves a one-to-one assignment problem: given a square cost matrix
/// between N agents and N tasks, finds the assignment that minimizes (or
/// maximizes) total cost, with each agent assigned exactly one task and
/// vice versa. Modeled as a binary-variable LP. Use for worker-to-task,
/// order-to-warehouse, or similar one-to-one matching problems.
pub fn solve_assignment(req: AssignmentRequest) -> AssignmentResponse {
    let n = req
        .cost_matrix
        .len();
    let direction = if req.maximize {
        OptimizationDirection::Maximize
    } else {
        OptimizationDirection::Minimize
    };
    let mut problem = Problem::new(direction);

    // One binary variable per (agent, task) pair, relaxed to [0, 1] --
    // the assignment polytope is integral for this problem structure, so
    // the LP relaxation itself yields an integral optimum here.
    let mut x = vec![vec![]; n];
    for i in 0..n {
        for j in 0..n {
            x[i].push(problem.add_var(req.cost_matrix[i][j], (0.0, 1.0)));
        }
    }

    for i in 0..n {
        let row: Vec<_> = (0..n)
            .map(|j| (x[i][j], 1.0))
            .collect();
        problem.add_constraint(row, ComparisonOp::Eq, 1.0);
    }
    for j in 0..n {
        let col: Vec<_> = (0..n)
            .map(|i| (x[i][j], 1.0))
            .collect();
        problem.add_constraint(col, ComparisonOp::Eq, 1.0);
    }

    match problem.solve() {
        Ok(microlp::SolveOutcome::Solution(solution)) => {
            let mut assignment = Vec::with_capacity(n);
            for i in 0..n {
                for j in 0..n {
                    if solution[x[i][j]] > 0.5 {
                        assignment.push((i, j));
                    }
                }
            }
            AssignmentResponse {
                status: "OPTIMAL".into(),
                total_cost: Some(solution.objective()),
                assignment: Some(assignment),
            }
        }
        _ => AssignmentResponse {
            status: "INFEASIBLE".into(),
            assignment: None,
            total_cost: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lp_optimal_minimize() {
        // minimize x + 2y subject to x + y == 10, x,y in [0,20].
        // Optimal puts all weight on the cheaper variable (x): x=10, y=0.
        let resp = solve_lp(LpRequest {
            objective: vec![1.0, 2.0],
            constraints: vec![LpConstraint {
                coeffs: vec![1.0, 1.0],
                op: "==".into(),
                rhs: 10.0,
            }],
            var_bounds: vec![(0.0, 20.0), (0.0, 20.0)],
        });
        assert_eq!(resp.status, "OPTIMAL");
        let values = resp
            .values
            .expect("expected variable values");
        assert!((values[0] - 10.0).abs() < 1e-6);
        assert!((values[1] - 0.0).abs() < 1e-6);
        assert!(
            (resp
                .objective_value
                .unwrap()
                - 10.0)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn lp_infeasible() {
        // x <= 1 and x >= 5 can't both hold.
        let resp = solve_lp(LpRequest {
            objective: vec![1.0],
            constraints: vec![
                LpConstraint {
                    coeffs: vec![1.0],
                    op: "<=".into(),
                    rhs: 1.0,
                },
                LpConstraint {
                    coeffs: vec![1.0],
                    op: ">=".into(),
                    rhs: 5.0,
                },
            ],
            var_bounds: vec![(0.0, 10.0)],
        });
        assert_eq!(resp.status, "INFEASIBLE");
        assert!(
            resp.values
                .is_none()
        );
        assert!(
            resp.objective_value
                .is_none()
        );
    }

    #[test]
    fn assignment_minimize() {
        let resp = solve_assignment(AssignmentRequest {
            cost_matrix: vec![vec![1.0, 2.0], vec![2.0, 1.0]],
            maximize: false,
        });
        assert_eq!(resp.status, "OPTIMAL");
        let mut assignment = resp
            .assignment
            .expect("expected an assignment");
        assignment.sort();
        assert_eq!(assignment, vec![(0, 0), (1, 1)]);
        assert!(
            (resp
                .total_cost
                .unwrap()
                - 2.0)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn assignment_maximize() {
        let resp = solve_assignment(AssignmentRequest {
            cost_matrix: vec![vec![1.0, 2.0], vec![2.0, 1.0]],
            maximize: true,
        });
        assert_eq!(resp.status, "OPTIMAL");
        let mut assignment = resp
            .assignment
            .expect("expected an assignment");
        assignment.sort();
        assert_eq!(assignment, vec![(0, 1), (1, 0)]);
        assert!(
            (resp
                .total_cost
                .unwrap()
                - 4.0)
                .abs()
                < 1e-6
        );
    }
}
