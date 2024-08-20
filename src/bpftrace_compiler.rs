use sqlparser::ast::*;
use std::result::Result;

fn parse_expr(e: &Expr) -> String {
    match e {
        Expr::Identifier(i) => i.value.clone(),
        Expr::Wildcard => "*".to_string(),
        Expr::Value(v) => v.to_string(),
        Expr::BinaryOp { left, op, right } => {
            format!("{} {} {}", parse_expr(left), op, parse_expr(right))
        }
        _ => "".to_string(),
    }
}

pub fn compile_ast_to_bpftrace(ast: Vec<Statement>) -> Result<(String, Vec<String>), &'static str> {
    let q = match &ast[0] {
        Statement::Query(q) => q,
        _ => return Err("Expected a query"),
    };
    let b = q.body.as_ref();

    let projections = match b {
        SetExpr::Select(s) => &s.projection,
        _ => return Err("Expected a select"),
    };

    let probe_relations = match b {
        SetExpr::Select(s) => &s.from,
        _ => return Err("Expected a select"),
    };

    let mut quick_exit = false;
    let probe_name = if probe_relations.is_empty() {
        quick_exit = true;
        "BEGIN".to_string()
    } else {
        let probes = probe_relations[0].clone().relation;
        let name = match &probes {
            TableFactor::Table { name, .. } => name,
            _ => return Err("Expected a table"),
        };
        //convert table name to probe name
        name.to_string().replace(".", ":")
    };

    // compile the query into bpftrace

    let mut bpftrace = String::new();

    //convert from into bpftrace probe
    bpftrace.push_str(&probe_name);
    bpftrace.push_str("\n {\n");

    // print out the projections

    let mut outputs = Vec::new();

    for projection in projections {
        match projection {
            SelectItem::UnnamedExpr(e) => {
                outputs.push(parse_expr(e));
            }
            _ => return Err("Expected an expression"),
        }
    }

    let mut results_update = String::new();

    results_update.push_str("@q1_id[\"id\"] = count();\n");

    for (i, e) in outputs.clone().into_iter().enumerate() {
        results_update.push_str(&format!("$q1_{} = {};\n", i, e));
    }

    bpftrace.push_str(&results_update);

    let mut print_str = String::new();

    print_str.push_str("print((");
    print_str.push_str("(\"id\",@q1_id[\"id\"]),");
    for (i,_) in outputs.clone().into_iter().enumerate() {
        print_str.push_str(&format!("({},$q1_{}),", i, i));
    }
    print_str.pop();
    print_str.push_str("));\n");

    bpftrace.push_str(&print_str);

    if quick_exit {
        bpftrace.push_str("exit();\n");
    }

    bpftrace.push_str(" }");

    Ok((bpftrace, outputs))
}
