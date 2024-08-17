use sqlparser::parser::Parser;
use sqlparser::dialect::GenericDialect;

pub fn parse_bpfquery_sql(sql: &str) -> std::vec::Vec<sqlparser::ast::Statement> {
    //eventually this will probably be more complicated 
    let dialect = GenericDialect {};  
    let ast = Parser::parse_sql(&dialect, sql).unwrap();
    return ast
}
