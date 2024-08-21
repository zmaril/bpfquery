use sqlparser::ast::*;
use std::result::Result;

fn parse_value(v: &Value) -> String {
    match v {
        Value::SingleQuotedString(s) => format!("\"{}\"", s.clone()),
        _ => v.to_string(),
    }
}

fn parse_fn_arg_expr(arg: &FunctionArgExpr) -> String {
    match arg {
        FunctionArgExpr::Expr(e) => parse_expr(e),
        FunctionArgExpr::Wildcard => "*".to_string(),
        FunctionArgExpr::QualifiedWildcard(o) => panic!("QualifiedWildcard not supported"),
    }
}

fn parse_fn_arg(arg: &FunctionArg) -> String {
    match arg {
        FunctionArg::Named {
            name,
            arg,
            operator: _,
        } => format!("{}={}", name, parse_fn_arg_expr(arg)), // no idea what operator is
        FunctionArg::Unnamed(e) => parse_fn_arg_expr(e),
    }
}

fn parse_expr(e: &Expr) -> String {
    match e {
        Expr::Identifier(i) => i.value.clone(),
        Expr::Wildcard => "*".to_string(),
        Expr::Value(v) => parse_value(v),
        Expr::BinaryOp { left, op, right } => {
            let mut ooop = op.to_string();
            // if it's an ==, we need to convert it to a =
            if ooop == "=" {
                ooop = "==".to_string();
            }

            format!("{} {} {}", parse_expr(left), ooop, parse_expr(right))
        }
        Expr::Function(f) => {
            let fns = f.name.to_string();
            match &f.args {
                FunctionArguments::List(fl) => {
                    let fargs = fl
                        .args
                        .iter()
                        .map(parse_fn_arg)
                        .collect::<Vec<String>>()
                        .join(",");
                    format!("{}({})", fns, fargs)
                }
                FunctionArguments::None => format!("{}()", fns),
                FunctionArguments::Subquery(q) => panic!("Subquery not supported"),
            }
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

    let select = match b {
        SetExpr::Select(s) => s,
        _ => return Err("Expected a select"),
    };

    let projections = &select.projection;
    let relations = &select.from;

    let mut quick_exit = false;
    let probe_name = if relations.is_empty() {
        quick_exit = true;
        "BEGIN".to_string()
    } else {
        let probes = relations[0].clone().relation;
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

    //add where/filter in, optional might not be there so we need to check
    let filters = match &select.selection {
        Some(e) => vec![e],
        None => vec![],
    };

    if !filters.is_empty() {
        bpftrace.push_str(" /");
        for filter in filters {
            bpftrace.push_str(&parse_expr(filter));
        }
        bpftrace.push_str("/ ");
    }

    bpftrace.push_str("\n {\n");

    // print out the projections

    let mut headers = Vec::new();
    let mut outputs = Vec::new();

    for projection in projections {
        match projection {
            SelectItem::UnnamedExpr(e) => {
                headers.push(e.to_string());
                outputs.push(parse_expr(e));
            }
            SelectItem::ExprWithAlias { expr, alias } => {
                headers.push(alias.value.clone());
                outputs.push(parse_expr(expr));
            }
            SelectItem::Wildcard(w) => {
                let opts = ["comm", "pid", "cpu", "elapsed"];
                for opt in opts.iter() {
                    headers.push(opt.to_string());
                    outputs.push(opt.to_string());
                }
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
    for (i, _) in outputs.clone().into_iter().enumerate() {
        print_str.push_str(&format!("({},$q1_{}),", i, i));
    }
    print_str.pop();
    print_str.push_str("));\n");

    bpftrace.push_str(&print_str);

    if quick_exit {
        bpftrace.push_str("exit();\n");
    }

    bpftrace.push_str(" }");
    Ok((bpftrace, headers))
}
