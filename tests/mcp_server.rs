//! End-to-end tests: spawn the compiled `solver-mcp` binary as a child
//! process and speak real MCP JSON-RPC to it over stdio, the same way an
//! agent host would. This is what catches wiring breakage (transport setup,
//! tool registration, result construction) that pure unit tests of the
//! solver functions in `csp_tools.rs` / `lp_tools.rs` can't see -- exactly
//! the class of bug introduced by the rmcp 3.x upgrade. Each tool gets one
//! happy-path call here; the solver-logic edge cases (infeasible, timeout,
//! etc.) are covered by the unit tests instead.

use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::transport::TokioChildProcess;
use rmcp::{RoleClient, ServiceExt};
use serde_json::{Value, json};
use tokio::process::Command;

async fn spawn_client() -> RunningService<RoleClient, ()> {
    let transport = TokioChildProcess::new(Command::new(env!("CARGO_BIN_EXE_solver-mcp")))
        .expect("failed to spawn solver-mcp");
    ().serve(transport)
        .await
        .expect("MCP initialize handshake failed")
}

async fn call_tool_json(
    client: &RunningService<RoleClient, ()>,
    name: &'static str,
    args: Value,
) -> Value {
    let result = client
        .call_tool(
            CallToolRequestParams::new(name).with_arguments(
                args.as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap_or_else(|e| panic!("tools/call {name} failed: {e}"));

    assert_ne!(
        result.is_error,
        Some(true),
        "{name} reported an error: {result:?}"
    );
    let text = result
        .content
        .iter()
        .find_map(|block| block.as_text())
        .unwrap_or_else(|| panic!("{name}: expected a text content block"));
    serde_json::from_str(&text.text)
        .unwrap_or_else(|e| panic!("{name}: output was not valid JSON: {e}"))
}

#[tokio::test]
async fn lists_all_tools() {
    let client = spawn_client().await;

    let tools = client
        .list_all_tools()
        .await
        .expect("tools/list failed");
    let names: Vec<&str> = tools
        .iter()
        .map(|t| {
            t.name
                .as_ref()
        })
        .collect();
    for expected in [
        "solve_csp",
        "solve_grouped_csp",
        "solve_scheduling",
        "solve_csp_ir",
        "solve_lp",
        "solve_assignment",
    ] {
        assert!(names.contains(&expected), "missing tool: {expected}");
    }

    client
        .cancel()
        .await
        .expect("clean shutdown failed");
}

#[tokio::test]
async fn calls_solve_csp() {
    let client = spawn_client().await;

    let parsed = call_tool_json(
        &client,
        "solve_csp",
        json!({
            "num_vars": 3,
            "domain_min": 0,
            "domain_max": 5,
            "all_different": true
        }),
    )
    .await;
    assert_eq!(parsed["status"], "SATISFIABLE");
    assert_eq!(
        parsed["assignment"]
            .as_array()
            .unwrap()
            .len(),
        3
    );

    client
        .cancel()
        .await
        .expect("clean shutdown failed");
}

#[tokio::test]
async fn calls_solve_grouped_csp() {
    let client = spawn_client().await;

    let parsed = call_tool_json(
        &client,
        "solve_grouped_csp",
        json!({
            "groups": [
                {"name": "Color", "values": ["Red", "Green", "Blue"]},
                {"name": "Pet", "values": ["Dog", "Cat", "Bird"]}
            ],
            "domain_min": 1,
            "domain_max": 3,
            "relations": [
                {"kind": "equals", "a": {"group": "Color", "value": "Red"}, "b": {"group": "Pet", "value": "Dog"}},
                {"kind": "constant", "a": {"group": "Color", "value": "Green"}, "value": 2}
            ]
        }),
    )
    .await;
    assert_eq!(parsed["status"], "SATISFIABLE");
    let color_red = parsed["assignment"]["Color"]["Red"]
        .as_i64()
        .unwrap();
    let pet_dog = parsed["assignment"]["Pet"]["Dog"]
        .as_i64()
        .unwrap();
    assert_eq!(color_red, pet_dog);
    assert_eq!(parsed["assignment"]["Color"]["Green"], 2);

    client
        .cancel()
        .await
        .expect("clean shutdown failed");
}

#[tokio::test]
async fn calls_solve_scheduling() {
    let client = spawn_client().await;

    let parsed = call_tool_json(
        &client,
        "solve_scheduling",
        json!({
            "durations": [2, 2],
            "demands": [1, 1],
            "capacity": 1,
            "horizon": 10
        }),
    )
    .await;
    assert_eq!(parsed["status"], "SATISFIABLE");
    assert_eq!(
        parsed["starts"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    client
        .cancel()
        .await
        .expect("clean shutdown failed");
}

#[tokio::test]
async fn calls_solve_csp_ir() {
    let client = spawn_client().await;

    // Same all-different-over-{0,1,2} problem as calls_solve_csp, expressed
    // in the general IR to check the transport/registration wiring for the
    // new tool specifically (solver-logic edge cases are covered by the
    // unit tests in csp_ir.rs).
    let parsed = call_tool_json(
        &client,
        "solve_csp_ir",
        json!({
            "variables": [
                {"kind": "int_range", "name": "a", "min": 0, "max": 2},
                {"kind": "int_range", "name": "b", "min": 0, "max": 2},
                {"kind": "int_range", "name": "c", "min": 0, "max": 2}
            ],
            "constraints": [
                {"kind": "all_different", "vars": [{"var": "a"}, {"var": "b"}, {"var": "c"}]}
            ],
            "solve": {"mode": "satisfy"}
        }),
    )
    .await;
    assert_eq!(parsed["status"], "SATISFIABLE");
    let assignment = parsed["assignment"]
        .as_object()
        .unwrap();
    assert_eq!(assignment.len(), 3);

    client
        .cancel()
        .await
        .expect("clean shutdown failed");
}

#[tokio::test]
async fn calls_solve_lp() {
    let client = spawn_client().await;

    let parsed = call_tool_json(
        &client,
        "solve_lp",
        json!({
            "objective": [1.0, 2.0],
            "constraints": [{"coeffs": [1.0, 1.0], "op": "==", "rhs": 10.0}],
            "var_bounds": [[0.0, 20.0], [0.0, 20.0]]
        }),
    )
    .await;
    assert_eq!(parsed["status"], "OPTIMAL");
    assert_eq!(parsed["objective_value"], 10.0);

    client
        .cancel()
        .await
        .expect("clean shutdown failed");
}

#[tokio::test]
async fn calls_solve_assignment() {
    let client = spawn_client().await;

    let parsed = call_tool_json(
        &client,
        "solve_assignment",
        json!({
            "cost_matrix": [[1.0, 2.0], [2.0, 1.0]],
            "maximize": false
        }),
    )
    .await;
    assert_eq!(parsed["status"], "OPTIMAL");
    assert_eq!(parsed["total_cost"], 2.0);

    client
        .cancel()
        .await
        .expect("clean shutdown failed");
}
