//! Query Executor module
//!
//! Executes SQL statements by coordinating parser, catalog, and heap storage.

use crate::catalog::Catalog;
use crate::heap::{HeapTable, Value};
use crate::sql::{self, Statement};
use crate::table::Column;
use crate::types::ColumnType;
use std::sync::Arc;

/// Executor result type
pub type ExecResult<T> = Result<T, ExecError>;

/// Executor error types
#[derive(Debug, Clone)]
pub enum ExecError {
    SqlError(sql::SqlError),
    TableNotFound(String),
    ColumnNotFound(String),
    Other(String),
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecError::SqlError(e) => write!(f, "SQL error: {}", e),
            ExecError::TableNotFound(name) => write!(f, "Table not found: {}", name),
            ExecError::ColumnNotFound(name) => write!(f, "Column not found: {}", name),
            ExecError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for ExecError {}

impl From<sql::SqlError> for ExecError {
    fn from(e: sql::SqlError) -> Self {
        ExecError::SqlError(e)
    }
}

/// Query executor that runs SQL statements
pub struct Executor {
    catalog: Arc<Catalog>,
    heap_tables: std::collections::HashMap<String, HeapTable>,
}

impl Executor {
    pub fn new(catalog: Arc<Catalog>) -> Self {
        Self {
            catalog,
            heap_tables: std::collections::HashMap::new(),
        }
    }

    /// Execute a SQL statement
    pub fn execute(&mut self, sql: &str) -> ExecResult<String> {
        let stmt = sql::parse(sql)?;

        match stmt {
            Statement::CreateTable(ct) => self.execute_create_table(ct),
            Statement::Insert(ins) => self.execute_insert(ins),
            Statement::Select(sel) => self.execute_select(sel),
            Statement::Update(upd) => self.execute_update(upd),
            Statement::Delete(del) => self.execute_delete(del),
        }
    }

    fn execute_create_table(&mut self, ct: sql::CreateTableStmt) -> ExecResult<String> {
        let columns: Vec<Column> = ct
            .columns
            .iter()
            .enumerate()
            .map(|(idx, col)| {
                Column::new(
                    col.name.clone(),
                    col.data_type.clone(),
                    col.nullable,
                    idx as u32,
                )
            })
            .collect();

        let table = self
            .catalog
            .create_table(&ct.table_name, 1, columns)
            .map_err(|e| ExecError::Other(e.to_string()))?;

        let heap_table = HeapTable::new(table, 1);
        self.heap_tables.insert(ct.table_name.clone(), heap_table);

        Ok(format!("Created table '{}'", ct.table_name))
    }

    fn execute_insert(&self, ins: sql::InsertStmt) -> ExecResult<String> {
        let count = ins.values.len();
        Ok(format!("INSERT: {} values provided", count))
    }

    fn execute_select(&self, sel: sql::SelectStmt) -> ExecResult<String> {
        if let Some(heap_table) = self.heap_tables.get(&sel.from) {
            let tuples = heap_table
                .scan()
                .map_err(|e| ExecError::Other(e.to_string()))?;

            if tuples.is_empty() {
                return Ok("(empty result)".to_string());
            }

            let mut output = String::new();
            for tuple in tuples {
                let vals: Vec<String> = tuple
                    .values()
                    .iter()
                    .map(|v| self.format_value(v))
                    .collect();
                output.push_str(&vals.join(" | "));
                output.push('\n');
            }
            return Ok(output);
        }

        Err(ExecError::TableNotFound(sel.from))
    }

    fn format_value(&self, val: &Value) -> String {
        match val {
            Value::Null => "NULL".to_string(),
            Value::Int8(n) => n.to_string(),
            Value::Int16(n) => n.to_string(),
            Value::Int32(n) => n.to_string(),
            Value::Int64(n) => n.to_string(),
            Value::UInt8(n) => n.to_string(),
            Value::UInt16(n) => n.to_string(),
            Value::UInt32(n) => n.to_string(),
            Value::UInt64(n) => n.to_string(),
            Value::Float32(n) => n.to_string(),
            Value::Float64(n) => n.to_string(),
            Value::Boolean(b) => b.to_string(),
            Value::VarChar(s) => s.clone(),
            Value::Blob(b) => format!("<{} bytes>", b.len()),
        }
    }

    fn execute_update(&self, upd: sql::UpdateStmt) -> ExecResult<String> {
        Ok(format!("UPDATE on '{}'", upd.table_name))
    }

    fn execute_delete(&self, del: sql::DeleteStmt) -> ExecResult<String> {
        Ok(format!("DELETE from '{}'", del.table_name))
    }
}
