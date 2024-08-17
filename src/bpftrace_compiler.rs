use sqlparser::ast::*;

pub fn compile_ast_to_bpftrace(ast: Vec<Statement>) -> String {
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
    bpftrace.push_str(" {");

    // print out the projections

    let mut outputs = Vec::new();    

    for projection in projections {
        match projection {
            SelectItem::UnnamedExpr(e) => {
                outputs.push(e);
            }
            _ => panic!("Expected an
            expression"),
        }
    }

    let mut printf = String::new();
    printf.push_str("printf(\"");
    for e in outputs.clone() {
        printf.push_str(& format!("{} %d ", e));
    }
    printf.push_str("\\n\", ");
    for e in outputs {
        printf.push_str(& format!("{}, ", e));
    }
    printf.pop();
    printf.pop();

    printf.push_str(");");

    bpftrace.push_str(&printf);


    bpftrace.push_str(" }");

    bpftrace
}