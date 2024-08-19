use sqlparser::parser::Parser;
use sqlparser::dialect::GenericDialect;

pub fn parse_bpfquery_sql(sql: &str) -> Result<Vec<sqlparser::ast::Statement>, sqlparser::parser::ParserError> {
    //eventually this will probably be more complicated 
    let dialect = GenericDialect {};  
    let ast = Parser::parse_sql(&dialect, sql);
    ast
}
