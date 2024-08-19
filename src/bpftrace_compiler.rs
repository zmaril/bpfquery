use sqlparser::ast::*;
use std::io::Result;


pub fn compile_ast_to_bpftrace(ast: Vec<Statement>) -> Result<(String, Vec<String>)> {
    let q = match &ast[0] {
        Statement::Query(q) => q,
        _ => panic!("Expected a query"),
    };
    let b = q.body.as_ref();

    let projections = match b {
        SetExpr::Select(s) => &s.projection,
        _ => panic!("Expected a select"),
    };


    let probe_relations = match b {
        SetExpr::Select(s) => &s.from,
        _ => panic!("Expected a select"),
    };


    let probes = probe_relations[0].clone().relation;
    let name = match &probes {
        TableFactor::Table { name, .. } => name,
        _ => panic!("Expected a table"),
    };
    //convert table name to probe name
    let probe_name = name.to_string().replace(".", ":");


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
                outputs.push(e.to_string());
            }
            _ => panic!("Expected an
            expression"),
        }
    }

    let mut results_update = String::new();


    results_update.push_str("\t@q1_id[\"id\"] = count();\n");

    for e in outputs.clone() {
        results_update.push_str(&format!("\t$q1_{} = {};\n", e, e));
    }

    //    print((("pid",$q1_pid), ("cpu", $q1_cpu ), ("elapsed", $q1_elapsed), ("id",@q1_id["id"]) ));

    bpftrace.push_str(&results_update);

    let mut print_str = String::new();

    print_str.push_str("\tprint((");
    print_str.push_str("(\"id\",@q1_id[\"id\"]),");
    for e in outputs.clone() {
        print_str.push_str(&format!("(\"{}\",$q1_{}),", e, e));
    }
    print_str.pop();
    print_str.push_str("));\n");

    bpftrace.push_str(&print_str);

    bpftrace.push_str(" }");

    Ok((bpftrace, outputs))
}