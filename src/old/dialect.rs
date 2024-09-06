use sqlparser::dialect::Dialect;

// A [`Dialect`] for [ClickHouse](https://clickhouse.com/).
#[derive(Debug)]
pub struct BPFTraceDialect {}

impl Dialect for BPFTraceDialect {
    fn is_identifier_start(&self, ch: char) -> bool {
        ch.is_ascii_lowercase() || ch.is_ascii_uppercase() || ch == '_' || ch == '*'
    }

    fn is_identifier_part(&self, ch: char) -> bool {
        self.is_identifier_start(ch) || ch.is_ascii_digit() || ch == '*'
    }
}
