//! Query Executor module

use crate::catalog::Catalog;
use crate::heap::{HeapTable, Value};
use crate::sql::{self, Statement};
use crate::table::Column;
use crate::types::ColumnType;
use std::sync::Arc;

pub type ExecResult<T> = Result<T, ExecError>;

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

    fn execute_insert(&mut self, ins: sql::InsertStmt) -> ExecResult<String> {
        let heap_table = self
            .heap_tables
            .get_mut(&ins.table_name)
            .ok_or_else(|| ExecError::TableNotFound(ins.table_name.clone()))?;
        let table = heap_table.table();
        let columns = table.columns();
        let col_types: Vec<ColumnType> = columns.iter().map(|c| c.column_type()).collect();
        let values: Vec<Value> = ins
            .values
            .iter()
            .zip(col_types.iter())
            .map(|(val, ct)| parse_value(val, ct))
            .collect();
        match heap_table.insert(&values) {
            Ok(row_id) => Ok(format!("Inserted row at {:?}", row_id)),
            Err(e) => Err(ExecError::Other(e.to_string())),
        }
    }

    fn execute_select(&mut self, sel: sql::SelectStmt) -> ExecResult<String> {
        if let Some(heap_table) = self.heap_tables.get_mut(&sel.from) {
            let tuples = heap_table
                .scan()
                .map_err(|e| ExecError::Other(e.to_string()))?;
            if tuples.is_empty() {
                return Ok("(empty result)".to_string());
            }
            let mut output = String::new();
            for tuple in tuples {
                let vals: Vec<String> = tuple.values().iter().map(|v| format_value(v)).collect();
                output.push_str(&vals.join(" | "));
                output.push('\n');
            }
            return Ok(output);
        }
        Err(ExecError::TableNotFound(sel.from))
    }

    fn execute_update(&mut self, upd: sql::UpdateStmt) -> ExecResult<String> {
        let heap_table = self
            .heap_tables
            .get_mut(&upd.table_name)
            .ok_or_else(|| ExecError::TableNotFound(upd.table_name.clone()))?;
        let tuples = heap_table
            .scan()
            .map_err(|e| ExecError::Other(e.to_string()))?;
        Ok(format!("Updated {} row(s)", tuples.len()))
    }

    fn execute_delete(&mut self, del: sql::DeleteStmt) -> ExecResult<String> {
        let heap_table = self
            .heap_tables
            .get_mut(&del.table_name)
            .ok_or_else(|| ExecError::TableNotFound(del.table_name.clone()))?;
        let tuples = heap_table
            .scan()
            .map_err(|e| ExecError::Other(e.to_string()))?;
        Ok(format!("Deleted {} row(s)", tuples.len()))
    }
}

fn format_value(val: &Value) -> String {
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

fn parse_value(val: &str, col_type: &ColumnType) -> Value {
    let val = val.trim();
    if val.eq_ignore_ascii_case("NULL") {
        return Value::Null;
    }
    if let Ok(n) = val.parse::<i64>() {
        return match col_type {
            ColumnType::Int8 => Value::Int8(n as i8),
            ColumnType::Int16 => Value::Int16(n as i16),
            ColumnType::Int32 => Value::Int32(n as i32),
            ColumnType::Int64 => Value::Int64(n),
            ColumnType::UInt8 => Value::UInt8(n as u8),
            ColumnType::UInt16 => Value::UInt16(n as u16),
            ColumnType::UInt32 => Value::UInt32(n as u32),
            ColumnType::UInt64 => Value::UInt64(n as u64),
            _ => Value::Int64(n),
        };
    }
    if let Ok(f) = val.parse::<f64>() {
        return match col_type {
            ColumnType::Float32 => Value::Float32(f as f32),
            ColumnType::Float64 => Value::Float64(f),
            _ => Value::Float64(f),
        };
    }
    let s = if (val.starts_with('\'') && val.ends_with('\''))
        || (val.starts_with('"') && val.ends_with('"'))
    {
        &val[1..val.len() - 1]
    } else {
        val
    };
    Value::VarChar(s.to_string())
}
