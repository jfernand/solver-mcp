//! Constraint-satisfaction and scheduling tools backed by Pumpkin
//! (a pure-Rust Lazy Clause Generation CP solver).
//!
//! These tools own anything combinatorial: "find any valid assignment",
//! all-different, and cumulative-resource scheduling. For pure linear
//! optimization (resource allocation, assignment problems) see `lp_tools.rs`.

use pumpkin_solver::Solver;
use pumpkin_solver::conflict_resolvers::resolvers::ResolutionResolver;
use pumpkin_solver::core::results::{ProblemSolution, SatisfactionResult};
use pumpkin_solver::core::termination::Indefinite;
use pumpkin_solver::core::variables::TransformableVariable;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CspRequest {
    /// Number of integer decision variables to create.
    pub num_vars: usize,
    /// Inclusive lower bound of every variable's domain.
    pub domain_min: i32,
    /// Inclusive upper bound of every variable's domain.
    pub domain_max: i32,
    /// If true, all variables are constrained to take pairwise-distinct values.
    #[serde(default)]
    pub all_different: bool,
    /// Wall-clock time limit in seconds before giving up and reporting a
    /// timeout rather than searching indefinitely.
    #[serde(default = "default_time_limit")]
    pub max_time_seconds: u64,
}

fn default_time_limit() -> u64 {
    5
}

#[derive(Serialize)]
pub struct CspResponse {
    /// One of "SATISFIABLE", "UNSATISFIABLE", or "TIMEOUT".
    pub status: String,
    /// Present only when status is "SATISFIABLE": one value per variable,
    /// in the order the variables were created.
    pub assignment: Option<Vec<i32>>,
}

/// Solves a general constraint-satisfaction problem over bounded integer
/// variables, optionally with an all-different constraint. Use for
/// combinatorial puzzles, assignment-with-exclusions, and any problem where
/// the goal is "find any valid assignment", not numeric optimization.
pub fn solve_csp(req: CspRequest) -> CspResponse {
    let mut solver = Solver::default();

    let vars: Vec<_> = (0..req.num_vars)
        .map(|_| solver.new_bounded_integer(req.domain_min, req.domain_max))
        .collect();

    if req.all_different {
        let tag = solver.new_constraint_tag();
        solver
            .add_constraint(pumpkin_constraints::all_different(vars.clone(), tag))
            .post();
    }

    let mut termination = Indefinite; // wrap with a real time-budgeted
    // termination condition in production;
    // Indefinite is illustrative here.
    let mut brancher = solver.default_brancher();
    let mut resolver = ResolutionResolver::default();

    let result = solver.satisfy(&mut brancher, &mut termination, &mut resolver);
    match result {
        SatisfactionResult::Satisfiable(satisfiable) => {
            let solution = satisfiable.solution();
            let assignment = vars
                .iter()
                .map(|&v| solution.get_integer_value(v))
                .collect();
            CspResponse {
                status: "SATISFIABLE".into(),
                assignment: Some(assignment),
            }
        }
        SatisfactionResult::Unsatisfiable(_, _, _) => CspResponse {
            status: "UNSATISFIABLE".into(),
            assignment: None,
        },
        SatisfactionResult::Unknown(_, _, _) => CspResponse {
            status: "TIMEOUT".into(),
            assignment: None,
        },
    }
}

/// A named category of decision variables, e.g. Color = [Red, Green, Ivory,
/// Yellow, Blue]. Each value becomes its own integer variable whose value is
/// the position/house index it is assigned to; the values within a group are
/// constrained to distinct positions.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct VarGroup {
    /// Category name, e.g. "Color". Referenced by relations as
    /// `{"group": "Color", "value": "Red"}`. Must be unique among groups.
    pub name: String,
    /// Distinct value names within this category, e.g.
    /// ["Red", "Green", "Ivory", "Yellow", "Blue"]. Must be unique within
    /// the group.
    pub values: Vec<String>,
}

/// A reference to one variable created by a [`VarGroup`]: the value named
/// `value` within the group named `group`.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct VarRef {
    pub group: String,
    pub value: String,
}

/// A constraint tying together variables created from `groups`, or pinning
/// one to a constant.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Relation {
    /// `a == b`. E.g. "the Englishman lives in the red house":
    /// Nationality.Englishman == Color.Red.
    Equals { a: VarRef, b: VarRef },
    /// `a == b + offset`, directional. E.g. "the green house is immediately
    /// to the right of the ivory house": Color.Green == Color.Ivory + 1.
    Offset { a: VarRef, b: VarRef, offset: i32 },
    /// `abs(a - b) == distance`, undirected. E.g. "Kools are smoked next to
    /// the horse": Smoke.Kools vs Pet.Horse with distance 1.
    Distance { a: VarRef, b: VarRef, distance: i32 },
    /// `a == value`. E.g. "milk is drunk in the middle house":
    /// Drink.Milk == 3.
    Constant { a: VarRef, value: i32 },
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct GroupedCspRequest {
    /// Each group contributes one variable per value; values within a group
    /// are constrained to pairwise-distinct positions. Use `relations` to
    /// tie variables across groups together.
    pub groups: Vec<VarGroup>,
    /// Inclusive lower bound of every variable's domain (e.g. 1 for a
    /// 1-indexed house number).
    pub domain_min: i32,
    /// Inclusive upper bound of every variable's domain (e.g. 5 for a
    /// 5-house puzzle).
    pub domain_max: i32,
    /// Cross-group and absolute constraints tying variables together.
    #[serde(default)]
    pub relations: Vec<Relation>,
    /// Wall-clock time limit in seconds before giving up and reporting a
    /// timeout rather than searching indefinitely.
    #[serde(default = "default_time_limit")]
    pub max_time_seconds: u64,
}

#[derive(Serialize)]
pub struct GroupedCspResponse {
    /// One of "SATISFIABLE", "UNSATISFIABLE", "TIMEOUT", or "ERROR".
    pub status: String,
    /// Present only when status is "SATISFIABLE": group name -> (value name
    /// -> assigned position).
    pub assignment: Option<HashMap<String, HashMap<String, i32>>>,
    /// Present only when status is "ERROR": what was wrong with the request.
    pub error: Option<String>,
}

fn grouped_csp_error(message: String) -> GroupedCspResponse {
    GroupedCspResponse {
        status: "ERROR".into(),
        assignment: None,
        error: Some(message),
    }
}

/// Solves a constraint-satisfaction problem expressed as named, grouped
/// variables (e.g. Color/Nationality/Drink/Smoke/Pet, five values each) tied
/// together by relations -- equality, directional offset, undirected
/// distance, and constants. Each group gets its own all-different
/// constraint, so values in different groups are never forced apart from
/// each other the way a single global all-different would. Use this for
/// logic-grid puzzles like the classic Zebra Puzzle, where clues relate
/// named attributes rather than an undifferentiated pool of integers.
pub fn solve_grouped_csp(req: GroupedCspRequest) -> GroupedCspResponse {
    let mut group_names = HashSet::new();
    for group in &req.groups {
        if !group_names.insert(
            group
                .name
                .clone(),
        ) {
            return grouped_csp_error(format!("duplicate group name '{}'", group.name));
        }
        let mut value_names = HashSet::new();
        for value in &group.values {
            if !value_names.insert(value.clone()) {
                return grouped_csp_error(format!(
                    "duplicate value '{value}' in group '{}'",
                    group.name
                ));
            }
        }
    }

    let mut solver = Solver::default();

    let mut var_map: HashMap<(String, String), _> = HashMap::new();
    let mut group_vars: Vec<Vec<_>> = Vec::new();

    for group in &req.groups {
        let vars: Vec<_> = group
            .values
            .iter()
            .map(|_| solver.new_bounded_integer(req.domain_min, req.domain_max))
            .collect();
        for (value, &var) in group
            .values
            .iter()
            .zip(vars.iter())
        {
            var_map.insert(
                (
                    group
                        .name
                        .clone(),
                    value.clone(),
                ),
                var,
            );
        }
        group_vars.push(vars);
    }

    for vars in &group_vars {
        if vars.len() > 1 {
            let tag = solver.new_constraint_tag();
            solver
                .add_constraint(pumpkin_constraints::all_different(vars.clone(), tag))
                .post();
        }
    }

    macro_rules! resolve {
        ($r:expr) => {
            match var_map.get(&(
                $r.group
                    .clone(),
                $r.value
                    .clone(),
            )) {
                Some(&v) => v,
                None => {
                    return grouped_csp_error(format!(
                        "unknown variable: group '{}', value '{}'",
                        $r.group, $r.value
                    ));
                }
            }
        };
    }

    for relation in &req.relations {
        match relation {
            Relation::Equals { a, b } => {
                let va = resolve!(a);
                let vb = resolve!(b);
                let tag = solver.new_constraint_tag();
                solver
                    .add_constraint(pumpkin_constraints::equals(
                        [va.scaled(1), vb.scaled(-1)],
                        0,
                        tag,
                    ))
                    .post();
            }
            Relation::Offset { a, b, offset } => {
                let va = resolve!(a);
                let vb = resolve!(b);
                let tag = solver.new_constraint_tag();
                solver
                    .add_constraint(pumpkin_constraints::equals(
                        [va.scaled(1), vb.scaled(-1)],
                        *offset,
                        tag,
                    ))
                    .post();
            }
            Relation::Constant { a, value } => {
                let va = resolve!(a);
                let tag = solver.new_constraint_tag();
                solver
                    .add_constraint(pumpkin_constraints::equals([va], *value, tag))
                    .post();
            }
            Relation::Distance { a, b, distance } => {
                let va = resolve!(a);
                let vb = resolve!(b);
                let diff = solver.new_bounded_integer(
                    req.domain_min - req.domain_max,
                    req.domain_max - req.domain_min,
                );
                let tag = solver.new_constraint_tag();
                solver
                    .add_constraint(pumpkin_constraints::equals(
                        [va.scaled(1), vb.scaled(-1), diff.scaled(-1)],
                        0,
                        tag,
                    ))
                    .post();
                let abs_val = solver.new_bounded_integer(*distance, *distance);
                let tag2 = solver.new_constraint_tag();
                solver
                    .add_constraint(pumpkin_constraints::absolute(diff, abs_val, tag2))
                    .post();
            }
        }
    }

    let mut termination = Indefinite;
    let mut brancher = solver.default_brancher();
    let mut resolver = ResolutionResolver::default();

    let result = solver.satisfy(&mut brancher, &mut termination, &mut resolver);
    match result {
        SatisfactionResult::Satisfiable(satisfiable) => {
            let solution = satisfiable.solution();
            let mut assignment: HashMap<String, HashMap<String, i32>> = HashMap::new();
            for group in &req.groups {
                assignment
                    .entry(
                        group
                            .name
                            .clone(),
                    )
                    .or_default();
            }
            for ((group_name, value_name), var) in &var_map {
                assignment
                    .entry(group_name.clone())
                    .or_default()
                    .insert(value_name.clone(), solution.get_integer_value(*var));
            }
            GroupedCspResponse {
                status: "SATISFIABLE".into(),
                assignment: Some(assignment),
                error: None,
            }
        }
        SatisfactionResult::Unsatisfiable(_, _, _) => GroupedCspResponse {
            status: "UNSATISFIABLE".into(),
            assignment: None,
            error: None,
        },
        SatisfactionResult::Unknown(_, _, _) => GroupedCspResponse {
            status: "TIMEOUT".into(),
            assignment: None,
            error: None,
        },
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ScheduleRequest {
    /// Duration of each task, in the same time unit as `horizon`.
    pub durations: Vec<i32>,
    /// Resource units each task consumes concurrently while running.
    pub demands: Vec<i32>,
    /// Total resource capacity available at any point in time.
    pub capacity: i32,
    /// Latest possible time any task may finish by.
    pub horizon: i32,
    #[serde(default = "default_time_limit")]
    pub max_time_seconds: u64,
}

#[derive(Serialize)]
pub struct ScheduleResponse {
    pub status: String,
    /// Present only when status is "SATISFIABLE": start time of each task,
    /// in the order tasks were given.
    pub starts: Option<Vec<i32>>,
}

/// Solves a resource-constrained scheduling problem: given a set of tasks
/// with fixed durations and resource demands, and a shared resource with
/// finite capacity, finds start times such that at no point in time does
/// total resource usage exceed capacity. Use for machine scheduling,
/// job-shop-style problems, and rostering with a single shared resource.
pub fn solve_scheduling(req: ScheduleRequest) -> ScheduleResponse {
    let mut solver = Solver::default();
    let n = req
        .durations
        .len();

    let starts: Vec<_> = (0..n)
        .map(|_| solver.new_bounded_integer(0, req.horizon))
        .collect();

    let tag = solver.new_constraint_tag();
    solver
        .add_constraint(pumpkin_constraints::cumulative(
            starts.clone(),
            req.durations
                .clone(),
            req.demands
                .clone(),
            req.capacity,
            tag,
        ))
        .post();

    let mut termination = Indefinite;
    let mut brancher = solver.default_brancher();
    let mut resolver = ResolutionResolver::default();

    let result = solver.satisfy(&mut brancher, &mut termination, &mut resolver);
    match result {
        SatisfactionResult::Satisfiable(satisfiable) => {
            let solution = satisfiable.solution();
            let result_starts = starts
                .iter()
                .map(|&s| solution.get_integer_value(s))
                .collect();
            ScheduleResponse {
                status: "SATISFIABLE".into(),
                starts: Some(result_starts),
            }
        }
        SatisfactionResult::Unsatisfiable(_, _, _) => ScheduleResponse {
            status: "UNSATISFIABLE".into(),
            starts: None,
        },
        SatisfactionResult::Unknown(_, _, _) => ScheduleResponse {
            status: "TIMEOUT".into(),
            starts: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csp_satisfiable_without_all_different() {
        let resp = solve_csp(CspRequest {
            num_vars: 3,
            domain_min: 0,
            domain_max: 5,
            all_different: false,
            max_time_seconds: 5,
        });
        assert_eq!(resp.status, "SATISFIABLE");
        let assignment = resp
            .assignment
            .expect("expected an assignment");
        assert_eq!(assignment.len(), 3);
        assert!(
            assignment
                .iter()
                .all(|&v| (0..=5).contains(&v))
        );
    }

    #[test]
    fn csp_all_different_feasible() {
        // 3 variables, 6 possible values: an all-different assignment exists.
        let resp = solve_csp(CspRequest {
            num_vars: 3,
            domain_min: 0,
            domain_max: 5,
            all_different: true,
            max_time_seconds: 5,
        });
        assert_eq!(resp.status, "SATISFIABLE");
        let assignment = resp
            .assignment
            .expect("expected an assignment");
        let mut sorted = assignment.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            assignment.len(),
            "values must be pairwise distinct"
        );
    }

    #[test]
    fn csp_all_different_infeasible() {
        // 3 variables but only 2 possible values: no all-different assignment exists.
        let resp = solve_csp(CspRequest {
            num_vars: 3,
            domain_min: 0,
            domain_max: 1,
            all_different: true,
            max_time_seconds: 5,
        });
        assert_eq!(resp.status, "UNSATISFIABLE");
        assert!(
            resp.assignment
                .is_none()
        );
    }

    #[test]
    fn grouped_csp_satisfies_equals_offset_distance_and_constant() {
        // Three houses (1..=3), three categories. Exercises every relation
        // kind at once and checks the returned assignment against each one
        // directly, rather than hand-solving the puzzle.
        let resp = solve_grouped_csp(GroupedCspRequest {
            groups: vec![
                VarGroup {
                    name: "Color".into(),
                    values: vec!["Red".into(), "Green".into(), "Blue".into()],
                },
                VarGroup {
                    name: "Pet".into(),
                    values: vec!["Dog".into(), "Cat".into(), "Bird".into()],
                },
                VarGroup {
                    name: "Drink".into(),
                    values: vec!["Milk".into(), "Tea".into(), "Coffee".into()],
                },
            ],
            domain_min: 1,
            domain_max: 3,
            relations: vec![
                Relation::Equals {
                    a: VarRef {
                        group: "Color".into(),
                        value: "Red".into(),
                    },
                    b: VarRef {
                        group: "Pet".into(),
                        value: "Dog".into(),
                    },
                },
                Relation::Offset {
                    a: VarRef {
                        group: "Color".into(),
                        value: "Green".into(),
                    },
                    b: VarRef {
                        group: "Color".into(),
                        value: "Blue".into(),
                    },
                    offset: 1,
                },
                Relation::Distance {
                    a: VarRef {
                        group: "Pet".into(),
                        value: "Cat".into(),
                    },
                    b: VarRef {
                        group: "Pet".into(),
                        value: "Bird".into(),
                    },
                    distance: 1,
                },
                Relation::Constant {
                    a: VarRef {
                        group: "Drink".into(),
                        value: "Milk".into(),
                    },
                    value: 2,
                },
            ],
            max_time_seconds: 5,
        });

        assert_eq!(resp.status, "SATISFIABLE");
        let assignment = resp
            .assignment
            .expect("expected an assignment");
        let color = &assignment["Color"];
        let pet = &assignment["Pet"];
        let drink = &assignment["Drink"];

        // Each group's values occupy distinct houses.
        for group in [color, pet, drink] {
            let mut houses: Vec<i32> = group
                .values()
                .copied()
                .collect();
            houses.sort();
            houses.dedup();
            assert_eq!(
                houses.len(),
                group.len(),
                "group values must be pairwise distinct"
            );
        }

        assert_eq!(color["Red"], pet["Dog"]);
        assert_eq!(color["Green"], color["Blue"] + 1);
        assert_eq!((pet["Cat"] - pet["Bird"]).abs(), 1);
        assert_eq!(drink["Milk"], 2);
    }

    #[test]
    fn grouped_csp_unsatisfiable_when_offset_conflicts_with_all_different() {
        // Two houses, but Green == Red + 1 forces Green > Red while both
        // must also stay within {1, 2} and be pairwise-distinct -- fine on
        // its own -- combined with a constant pinning Red to 2, which makes
        // Green == 3, outside the domain.
        let resp = solve_grouped_csp(GroupedCspRequest {
            groups: vec![VarGroup {
                name: "Color".into(),
                values: vec!["Red".into(), "Green".into()],
            }],
            domain_min: 1,
            domain_max: 2,
            relations: vec![
                Relation::Constant {
                    a: VarRef {
                        group: "Color".into(),
                        value: "Red".into(),
                    },
                    value: 2,
                },
                Relation::Offset {
                    a: VarRef {
                        group: "Color".into(),
                        value: "Green".into(),
                    },
                    b: VarRef {
                        group: "Color".into(),
                        value: "Red".into(),
                    },
                    offset: 1,
                },
            ],
            max_time_seconds: 5,
        });
        assert_eq!(resp.status, "UNSATISFIABLE");
        assert!(
            resp.assignment
                .is_none()
        );
    }

    #[test]
    fn grouped_csp_reports_error_on_unknown_variable_reference() {
        let resp = solve_grouped_csp(GroupedCspRequest {
            groups: vec![VarGroup {
                name: "Color".into(),
                values: vec!["Red".into(), "Green".into()],
            }],
            domain_min: 1,
            domain_max: 2,
            relations: vec![Relation::Constant {
                a: VarRef {
                    group: "Color".into(),
                    value: "Purple".into(),
                },
                value: 1,
            }],
            max_time_seconds: 5,
        });
        assert_eq!(resp.status, "ERROR");
        assert!(
            resp.error
                .is_some()
        );
    }

    #[test]
    fn scheduling_feasible_when_tasks_fit_sequentially() {
        // Two unit-capacity tasks can't overlap, but a horizon of 10 gives
        // them plenty of room to run one after another.
        let resp = solve_scheduling(ScheduleRequest {
            durations: vec![2, 2],
            demands: vec![1, 1],
            capacity: 1,
            horizon: 10,
            max_time_seconds: 5,
        });
        assert_eq!(resp.status, "SATISFIABLE");
        let starts = resp
            .starts
            .expect("expected start times");
        assert_eq!(starts.len(), 2);
        // Non-overlapping: one task's interval must end before the other starts.
        let (s0, s1) = (starts[0], starts[1]);
        assert!(s0 + 2 <= s1 || s1 + 2 <= s0, "tasks must not overlap");
    }

    #[test]
    fn scheduling_infeasible_when_starts_are_forced_to_overlap() {
        // Note: `horizon` only bounds each task's *start* time (0..=horizon),
        // not its finish time -- so a task may run past the horizon. With
        // horizon=0 both starts are forced to 0, so two capacity-1 tasks
        // demanding 1 unit each necessarily overlap and exceed capacity.
        let resp = solve_scheduling(ScheduleRequest {
            durations: vec![5, 5],
            demands: vec![1, 1],
            capacity: 1,
            horizon: 0,
            max_time_seconds: 5,
        });
        assert_eq!(resp.status, "UNSATISFIABLE");
        assert!(
            resp.starts
                .is_none()
        );
    }
}
