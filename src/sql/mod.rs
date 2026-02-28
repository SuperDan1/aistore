//! SQL Parser module
//!
//! Provides SQL parsing for basic DML and DDL statements.

use crate::types::ColumnType;

/// SQL result type
pub type SqlResult<T> = Result<T, SqlError>;

/// SQL error types
#[derive(Debug, Clone)]
pub enum SqlError {
    SyntaxError(String),
    ParseError(String),
}

impl std::fmt::Display for SqlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SqlError::SyntaxError(msg) => write!(f, "Syntax error: {}", msg),
            SqlError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl std::error::Error for SqlError {}

/// SQL AST nodes
#[derive(Debug, Clone)]
pub enum Statement {
    CreateTable(CreateTableStmt),
    Insert(InsertStmt),
    Select(SelectStmt),
    Update(UpdateStmt),
    Delete(DeleteStmt),
}

#[derive(Debug, Clone)]
pub struct CreateTableStmt {
    pub table_name: String,
    pub columns: Vec<ColumnDef>,
}

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: ColumnType,
    pub nullable: bool,
}

#[derive(Debug, Clone)]
pub struct InsertStmt {
    pub table_name: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SelectStmt {
    pub from: String,
    pub where_clause: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateStmt {
    pub table_name: String,
    pub set: Vec<(String, String)>,
    pub where_clause: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeleteStmt {
    pub table_name: String,
    pub where_clause: Option<String>,
}

/// Parse SQL string into AST
pub fn parse(sql: &str) -> SqlResult<Statement> {
    let sql = sql.trim();

    if sql.to_uppercase().starts_with("CREATE TABLE") {
        parse_create_table(sql)
    } else if sql.to_uppercase().starts_with("INSERT") {
        parse_insert(sql)
    } else if sql.to_uppercase().starts_with("SELECT") {
        parse_select(sql)
    } else if sql.to_uppercase().starts_with("UPDATE") {
        parse_update(sql)
    } else if sql.to_uppercase().starts_with("DELETE") {
        parse_delete(sql)
    } else {
        Err(SqlError::SyntaxError("Unknown SQL statement".to_string()))
    }
}

fn parse_create_table(sql: &str) -> SqlResult<Statement> {
    // Extract table name
    let after_create = sql["CREATE TABLE".len()..].trim();
    let paren_pos = after_create
        .find('(')
        .ok_or_else(|| SqlError::SyntaxError("Expected (".to_string()))?;
    let table_name = after_create[..paren_pos].trim().to_string();

    // Extract column definitions
    let paren_end = after_create
        .rfind(')')
        .ok_or_else(|| SqlError::SyntaxError("Expected )".to_string()))?;
    let columns_str = &after_create[paren_pos + 1..paren_end];

    let mut columns = Vec::new();
    for col_def in columns_str.split(',') {
        let col_def = col_def.trim();
        if col_def.is_empty() {
            continue;
        }

        let parts: Vec<&str> = col_def.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let name = parts[0].to_string();
        let data_type = parse_type(parts.get(1).unwrap_or(&"INT"))?;
        let nullable = !col_def.to_uppercase().contains("NOT NULL");

        columns.push(ColumnDef {
            name,
            data_type,
            nullable,
        });
    }

    Ok(Statement::CreateTable(CreateTableStmt {
        table_name,
        columns,
    }))
}

fn parse_type(type_str: &str) -> SqlResult<ColumnType> {
    let upper = type_str.to_uppercase();
    let upper = upper.trim_end_matches('(').trim_end_matches(')');

    // Check for VARCHAR(n) or BLOB(n)
    if let Some(pos) = upper.find('(') {
        let base = &upper[..pos];
        let rest = &upper[pos + 1..];
        if let Some(size_str) = rest.find(')') {
            let size: u32 = rest[..size_str]
                .parse()
                .map_err(|_| SqlError::ParseError("Invalid size".to_string()))?;
            match base {
                "VARCHAR" => return Ok(ColumnType::Varchar(size)),
                "BLOB" => return Ok(ColumnType::Blob(size)),
                _ => {}
            }
        }
    }

    match upper {
        "INT" | "INT32" | "INTEGER" => Ok(ColumnType::Int32),
        "INT64" | "BIGINT" => Ok(ColumnType::Int64),
        "INT16" | "SMALLINT" => Ok(ColumnType::Int16),
        "INT8" | "TINYINT" => Ok(ColumnType::Int8),
        "FLOAT" | "FLOAT32" => Ok(ColumnType::Float32),
        "DOUBLE" | "FLOAT64" => Ok(ColumnType::Float64),
        "BOOL" | "BOOLEAN" => Ok(ColumnType::Bool),
        "TEXT" => Ok(ColumnType::Varchar(255)),
        _ => Ok(ColumnType::Int32),
    }
}

fn parse_insert(sql: &str) -> SqlResult<Statement> {
    let after_insert = sql["INSERT".len()..].trim();
    let after_into = if after_insert.to_uppercase().starts_with("INTO") {
        after_insert["INTO".len()..].trim()
    } else {
        after_insert
    };

    let space_pos = after_into
        .find(|c: char| c.is_whitespace())
        .unwrap_or(after_into.len());
    let table_name = after_into[..space_pos].trim().to_string();

    let values_str = after_into[space_pos..].trim();
    let values: Vec<String> = values_str
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    Ok(Statement::Insert(InsertStmt { table_name, values }))
}

fn parse_select(sql: &str) -> SqlResult<Statement> {
    let after_select = sql["SELECT".len()..].trim();

    let from_pos = after_select
        .to_uppercase()
        .find("FROM")
        .ok_or_else(|| SqlError::SyntaxError("Expected FROM".to_string()))?;
    let _columns = after_select[..from_pos].trim();
    let after_from = after_select[from_pos + "FROM".len()..].trim();

    let where_pos = after_from.to_uppercase().find("WHERE");
    let (from, where_clause) = match where_pos {
        Some(pos) => (
            after_from[..pos].trim().to_string(),
            Some(after_from[pos + "WHERE".len()..].trim().to_string()),
        ),
        None => (after_from.trim().to_string(), None),
    };

    Ok(Statement::Select(SelectStmt { from, where_clause }))
}

fn parse_update(sql: &str) -> SqlResult<Statement> {
    let after_update = sql["UPDATE".len()..].trim();

    let set_pos = after_update
        .to_uppercase()
        .find("SET")
        .ok_or_else(|| SqlError::SyntaxError("Expected SET".to_string()))?;
    let table_name = after_update[..set_pos].trim().to_string();
    let after_set = after_update[set_pos + "SET".len()..].trim();

    let where_pos = after_set.to_uppercase().find("WHERE");
    let (set_str, where_clause) = match where_pos {
        Some(pos) => (
            after_set[..pos].trim().to_string(),
            Some(after_set[pos + "WHERE".len()..].trim().to_string()),
        ),
        None => (after_set.trim().to_string(), None),
    };

    let set: Vec<(String, String)> = set_str
        .split(',')
        .filter_map(|s| {
            let s = s.trim();
            let eq_pos = s.find('=')?;
            Some((
                s[..eq_pos].trim().to_string(),
                s[eq_pos + 1..].trim().to_string(),
            ))
        })
        .collect();

    Ok(Statement::Update(UpdateStmt {
        table_name,
        set,
        where_clause,
    }))
}

fn parse_delete(sql: &str) -> SqlResult<Statement> {
    let after_delete = sql["DELETE".len()..].trim();

    let from_pos = after_delete
        .to_uppercase()
        .find("FROM")
        .ok_or_else(|| SqlError::SyntaxError("Expected FROM".to_string()))?;
    let after_from = after_delete[from_pos + "FROM".len()..].trim();

    let where_pos = after_from.to_uppercase().find("WHERE");
    let (table_name, where_clause) = match where_pos {
        Some(pos) => (
            after_from[..pos].trim().to_string(),
            Some(after_from[pos + "WHERE".len()..].trim().to_string()),
        ),
        None => (after_from.trim().to_string(), None),
    };

    Ok(Statement::Delete(DeleteStmt {
        table_name,
        where_clause,
    }))
}
