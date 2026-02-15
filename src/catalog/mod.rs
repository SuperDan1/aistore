use crate::catalog::error::{CatalogError, CatalogResult};
use crate::table::{Column, Table, TableBuilder, TableType};
use crate::types::SegmentId;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub mod error;

#[derive(Debug, Clone)]
struct TableEntry {
    table: Arc<Table>,
}

pub struct Catalog {
    data_dir: PathBuf,
    system_dir: PathBuf,
    column_table_path: PathBuf,
    name_cache: RwLock<HashMap<String, TableEntry>>,
    id_cache: RwLock<HashMap<u64, String>>,
    next_table_id: RwLock<u64>,
}

impl Catalog {
    const SYSTEM_DIR: &'static str = "system";
    const COLUMN_TABLE_FILE: &'static str = "columns.dat";
    const TABLE_FILE_EXT: &'static str = ".tbl";

    pub fn new(data_dir: impl AsRef<Path>) -> CatalogResult<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        let system_dir = data_dir.join(Self::SYSTEM_DIR);
        let column_table_path = system_dir.join(Self::COLUMN_TABLE_FILE);

        fs::create_dir_all(&data_dir)?;
        fs::create_dir_all(&system_dir)?;

        Ok(Self {
            data_dir,
            system_dir,
            column_table_path,
            name_cache: RwLock::new(HashMap::new()),
            id_cache: RwLock::new(HashMap::new()),
            next_table_id: RwLock::new(1),
        })
    }

    pub fn load(data_dir: impl AsRef<Path>) -> CatalogResult<Self> {
        let catalog = Self::new(data_dir)?;

        if catalog.system_dir.exists() {
            for entry in fs::read_dir(&catalog.system_dir)? {
                let entry = entry?;
                let path = entry.path();

                if let Some(ext) = path.extension() {
                    if ext == "tbl" {
                        if let Some(table) = catalog.parse_table_file(&path)? {
                            catalog.add_to_cache(table)?;
                        }
                    }
                }
            }
        }

        Ok(catalog)
    }

    pub fn create_table(
        &self,
        table_name: &str,
        segment_id: SegmentId,
        columns: Vec<Column>,
    ) -> CatalogResult<Arc<Table>> {
        if self.name_cache.read().contains_key(table_name) {
            return Err(CatalogError::TableAlreadyExists(table_name.to_string()));
        }

        let table_id = self.allocate_table_id();

        let mut col_names = HashSet::new();
        for col in &columns {
            if !col_names.insert(col.name()) {
                return Err(CatalogError::ColumnAlreadyExists(col.name().to_string()));
            }
        }

        let table = TableBuilder::new(table_id, table_name.to_string())
            .segment_id(segment_id)
            .table_type(TableType::User)
            .columns(columns)
            .try_build()
            .map_err(|e| CatalogError::InvalidArgument(e))?;

        let table_arc = Arc::new(table);

        self.persist_table(&table_arc)?;
        self.add_to_cache(table_arc.clone())?;

        Ok(table_arc)
    }

    pub fn get_table(&self, table_name: &str) -> CatalogResult<Arc<Table>> {
        self.name_cache
            .read()
            .get(table_name)
            .map(|entry| entry.table.clone())
            .ok_or_else(|| CatalogError::TableNotFound(table_name.to_string()))
    }

    pub fn get_table_by_id(&self, table_id: u64) -> CatalogResult<Arc<Table>> {
        let name = {
            let id_cache = self.id_cache.read();
            id_cache
                .get(&table_id)
                .ok_or_else(|| CatalogError::TableNotFound(format!("ID: {}", table_id)))?
                .clone()
        };

        self.get_table(&name)
    }

    pub fn list_tables(&self) -> Vec<Arc<Table>> {
        self.name_cache
            .read()
            .values()
            .map(|entry| entry.table.clone())
            .collect()
    }

    pub fn drop_table(&self, table_name: &str) -> CatalogResult<()> {
        let _table_id = {
            let mut name_cache = self.name_cache.write();
            let entry = name_cache
                .remove(table_name)
                .ok_or_else(|| CatalogError::TableNotFound(table_name.to_string()))?;

            let mut id_cache = self.id_cache.write();
            id_cache.remove(&entry.table.table_id);

            entry.table.table_id
        };

        let table_file = self.get_table_file_path(table_name);
        if table_file.exists() {
            fs::remove_file(&table_file)?;
        }

        Ok(())
    }

    pub fn table_exists(&self, table_name: &str) -> bool {
        self.name_cache.read().contains_key(table_name)
    }

    pub fn peek_next_table_id(&self) -> u64 {
        *self.next_table_id.read()
    }

    fn allocate_table_id(&self) -> u64 {
        let mut next_id = self.next_table_id.write();
        let id = *next_id;
        *next_id += 1;
        id
    }

    fn add_to_cache(&self, table: Arc<Table>) -> CatalogResult<()> {
        let entry = TableEntry {
            table: table.clone(),
        };

        let mut name_cache = self.name_cache.write();
        let mut id_cache = self.id_cache.write();

        if name_cache.contains_key(table.table_name()) {
            return Err(CatalogError::TableAlreadyExists(
                table.table_name().to_string(),
            ));
        }

        name_cache.insert(table.table_name().to_string(), entry);
        id_cache.insert(table.table_id, table.table_name().to_string());

        let mut next_id = self.next_table_id.write();
        if table.table_id >= *next_id {
            *next_id = table.table_id + 1;
        }

        Ok(())
    }

    fn get_table_file_path(&self, table_name: &str) -> PathBuf {
        self.system_dir
            .join(format!("{}{}", table_name, Self::TABLE_FILE_EXT))
    }

    fn persist_table(&self, table: &Table) -> CatalogResult<()> {
        let table_file = self.get_table_file_path(table.table_name());
        let mut content = String::new();

        content.push_str(&format!(
            "{}|{}|{}|{:?}|{}|{}|{}\n",
            table.table_id,
            table.table_name,
            table.segment_id,
            table.table_type,
            table.row_count,
            table.column_count,
            table.created_at
        ));

        for col in table.columns() {
            content.push_str(&format!(
                "COLUMN|{}|{:?}|{}|{}\n",
                col.name(),
                col.column_type(),
                col.is_nullable(),
                col.ordinal()
            ));
        }

        fs::write(&table_file, content)?;
        Ok(())
    }

    fn parse_table_file(&self, path: &Path) -> CatalogResult<Option<Arc<Table>>> {
        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        let header = match lines.next() {
            Some(Ok(line)) => line,
            _ => return Ok(None),
        };

        let parts: Vec<&str> = header.split('|').collect();
        if parts.len() != 7 {
            return Err(CatalogError::ParseError(format!(
                "Invalid table file format: {:?}",
                path
            )));
        }

        let table_id: u64 = parts[0]
            .parse::<u64>()
            .map_err(|e| CatalogError::ParseError(e.to_string()))?;
        let table_name = parts[1].to_string();
        let segment_id: u64 = parts[2]
            .parse::<u64>()
            .map_err(|e| CatalogError::ParseError(e.to_string()))?;
        let table_type = match parts[3] {
            "User" => TableType::User,
            "System" => TableType::System,
            "Temporary" => TableType::Temporary,
            _ => TableType::User,
        };
        let row_count: u64 = parts[4]
            .parse::<u64>()
            .map_err(|e| CatalogError::ParseError(e.to_string()))?;
        let _column_count: u32 = parts[5]
            .parse::<u32>()
            .map_err(|e| CatalogError::ParseError(e.to_string()))?;
        let created_at: u64 = parts[6]
            .parse::<u64>()
            .map_err(|e| CatalogError::ParseError(e.to_string()))?;

        let mut columns = Vec::new();
        for line in lines {
            let line = line?;
            if line.starts_with("COLUMN|") {
                let col_parts: Vec<&str> = line.split('|').collect();
                if col_parts.len() != 5 {
                    continue;
                }

                let col_name = col_parts[1].to_string();
                let col_type = Self::parse_column_type(col_parts[2])?;
                let nullable = col_parts[3] == "true";
                let ordinal: u32 = col_parts[4]
                    .parse::<u32>()
                    .map_err(|e| CatalogError::ParseError(e.to_string()))?;

                columns.push(Column::new(col_name, col_type, nullable, ordinal));
            }
        }

        let mut table = Table::with_type(table_id, table_name, segment_id, table_type);
        table.set_columns(columns);
        table.row_count = row_count;
        table.created_at = created_at;

        Ok(Some(Arc::new(table)))
    }

    fn parse_column_type(type_str: &str) -> CatalogResult<crate::types::ColumnType> {
        match type_str {
            "Int8" => Ok(crate::types::ColumnType::Int8),
            "Int16" => Ok(crate::types::ColumnType::Int16),
            "Int32" => Ok(crate::types::ColumnType::Int32),
            "Int64" => Ok(crate::types::ColumnType::Int64),
            "UInt8" => Ok(crate::types::ColumnType::UInt8),
            "UInt16" => Ok(crate::types::ColumnType::UInt16),
            "UInt32" => Ok(crate::types::ColumnType::UInt32),
            "UInt64" => Ok(crate::types::ColumnType::UInt64),
            "Float32" => Ok(crate::types::ColumnType::Float32),
            "Float64" => Ok(crate::types::ColumnType::Float64),
            "Bool" => Ok(crate::types::ColumnType::Bool),
            _ => {
                if let Some(start) = type_str.find('(') {
                    let end = type_str.find(')').ok_or_else(|| {
                        CatalogError::ParseError(format!("Invalid type format: {}", type_str))
                    })?;
                    let type_name = &type_str[..start];
                    let param: u32 = type_str[start + 1..end]
                        .parse::<u32>()
                        .map_err(|e| CatalogError::ParseError(e.to_string()))?;

                    match type_name {
                        "Varchar" => Ok(crate::types::ColumnType::Varchar(param)),
                        "Blob" => Ok(crate::types::ColumnType::Blob(param)),
                        _ => Err(CatalogError::ParseError(format!(
                            "Unknown type: {}",
                            type_name
                        ))),
                    }
                } else {
                    Err(CatalogError::ParseError(format!(
                        "Unknown type: {}",
                        type_str
                    )))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    include!("tests.rs");
}
