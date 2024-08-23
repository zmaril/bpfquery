use rusqlite::{params, Connection, Result};
use sqlparser::ast::*;

fn get_struct_for_arg(function_name: &String, arg_name: &String) -> String {
    let conn = Connection::open("linux_kernel_definitions.db").unwrap();

    let mut stmt = conn
        .prepare("SELECT signature from function where function_name = ?")
        .unwrap();

    // Iterate over each string and check if it exists in the database
    let sig: String = stmt.query_row(params![function_name], |row| row.get(0)).unwrap();
    //((const struct path * path,struct file * file)
    // break this up by comma, then by space, find the one that matches the arg_name with the last element, and save the rest of the string
    let cleaned = sig.replace("(", "").replace(")", "").replace("const", "");
    let sigs = cleaned.split(",").collect::<Vec<&str>>();
    let mut structs = "".to_string();
    let mut index = 0;
    for (i,s) in sigs.into_iter().enumerate() {
        let ss = s.trim().split(" ").collect::<Vec<&str>>();
        if ss[ss.len() - 1] == arg_name {
            //everyhting but the last element
            structs = ss.iter().take(ss.len() - 1).map(|x| x.to_string()).collect::<Vec<String>>().join(" ");
            index = i;
            break;
        }
    }
    format!("(({})arg{})", structs, index)
}

fn resolve_compound_identifier(cs: &Vec<Ident>, relation: &String) -> String {
    // if the first ident is args, then we do a lookup in the database for 
    
    //only do this for kprobes for now
    let rs = relation.split(":").collect::<Vec<&str>>();
    let probe_type = rs[0];
    let probe_name = rs[1];

    if cs[0].value == "args" && probe_type == "kprobe" {
        let arg_name = cs[1].value.clone();
        get_struct_for_arg(&probe_name.to_string(), &arg_name)
        //still need to figure out what happens if you have a struct of a struct
    }
    else {
    cs.iter()
        .map(|i| (i.value.clone()))
        .collect::<Vec<String>>()
        .join(".")

    } 
}

fn parse_value(v: &Value) -> String {
    match v {
        Value::SingleQuotedString(s) => format!("\"{}\"", s.clone()),
        _ => v.to_string(),
    }
}

fn parse_fn_arg_expr(arg: &FunctionArgExpr, relation: &String ) -> String {
    match arg {
        FunctionArgExpr::Expr(e) => parse_expr(e, relation),
        FunctionArgExpr::Wildcard => "*".to_string(),
        FunctionArgExpr::QualifiedWildcard(_o) => panic!("QualifiedWildcard not supported"),
    }
}

fn parse_fn_arg(arg: &FunctionArg, relation: &String) -> String {
    match arg {
        FunctionArg::Named {
            name,
            arg,
            operator: _,
        } => format!("{}={}", name, parse_fn_arg_expr(arg, relation)), // no idea what operator is
        FunctionArg::Unnamed(e) => parse_fn_arg_expr(e, relation),
    }
}

fn parse_expr(e: &Expr, relation: &String) -> String {
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

            format!(
                "{} {} {}",
                parse_expr(left, relation),
                ooop,
                parse_expr(right, relation)
            )
        }
        Expr::Function(f) => {
            let fns = f.name.to_string();
            match &f.args {
                FunctionArguments::List(fl) => {
                    let fargs = fl
                        .args
                        .iter()
                        .map(|x| parse_fn_arg(x, relation))
                        .collect::<Vec<String>>()
                        .join(",");
                    format!("{}({})", fns, fargs)
                }
                FunctionArguments::None => format!("{}()", fns),
                FunctionArguments::Subquery(q) => panic!("Subquery not supported"),
            }
        }
        Expr::CompoundIdentifier(c) => resolve_compound_identifier(c, relation),
        v => panic!("Unsupported expression: {:?}", v),
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
            bpftrace.push_str(&parse_expr(filter, &probe_name));
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
                outputs.push(parse_expr(e, &probe_name));
            }
            SelectItem::ExprWithAlias { expr, alias } => {
                headers.push(alias.value.clone());
                outputs.push(parse_expr(expr, &probe_name));
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
