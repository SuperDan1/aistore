//! Benchmark scenarios module

use aistore::heap::{RowId, Value};
use aistore::storage::{Filter, StorageEngine};
use rand::Rng;
use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};



/// Scenario trait - defines a benchmark scenario
pub trait Scenario: Send + Sync {
    /// Prepare scenario (create tables, pre-populate data)
    fn prepare(&self, storage: &mut StorageEngine, rows: usize) -> Result<(), Box<dyn Error>>;

    /// Execute one iteration of the scenario
    fn execute(
        &self,
        storage: &mut StorageEngine,
        rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>>;

    /// Get table name
    fn table_name(&self) -> &str;
}

/// Point select scenario - single row lookup by primary key
pub struct PointSelect {
    table_name: String,
    rows: usize,
}

impl PointSelect {
    pub fn new(_tables: usize, rows: usize) -> Self {
        Self {
            table_name: "sbtest1".to_string(),
            rows,
        }
    }
}

impl Scenario for PointSelect {
    fn prepare(&self, storage: &mut StorageEngine, _rows: usize) -> Result<(), Box<dyn Error>> {
        // Table already created in main
        // Pre-populate with some data
        for i in 1..=100.min(self.rows) {
            let values = vec![
                Value::Int64(i as i64),
                Value::Int64(i as i64),
                Value::VarChar(format!("data_{}", i)),
                Value::VarChar(format!("pad_{}", i)),
            ];
            storage.insert(&self.table_name, values)?;
        }
        Ok(())
    }

    fn execute(
        &self,
        storage: &mut StorageEngine,
        rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>> {
        let id = rng.gen_range(1..=self.rows.min(100)) as i64;
        // Use filter to get matching rows directly
        let filter = Filter {
            column: "id".to_string(),
            value: Value::Int64(id),
        };
        let _tuples = storage.scan(&self.table_name, Some(filter))?;
        Ok(())
    }

    fn table_name(&self) -> &str {
        &self.table_name
    }
        }

/// Read only scenario - simple SELECT

/// Read only scenario - simple SELECT
pub struct ReadOnly {
    table_name: String,
}

impl ReadOnly {
    pub fn new(_tables: usize, _rows: usize) -> Self {
        Self {
            table_name: "sbtest1".to_string(),
        }
    }
}

impl Scenario for ReadOnly {
    fn prepare(&self, storage: &mut StorageEngine, rows: usize) -> Result<(), Box<dyn Error>> {
        for i in 1..=rows.min(100) {
            let values = vec![
                Value::Int64(i as i64),
                Value::Int64(i as i64),
                Value::VarChar(format!("data_{}", i)),
                Value::VarChar(format!("pad_{}", i)),
            ];
            storage.insert(&self.table_name, values)?;
        }
        Ok(())
    }

    fn execute(
        &self,
        storage: &mut StorageEngine,
        _rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>> {
        let _tuples = storage.scan(&self.table_name, None)?;
        Ok(())
    }

    fn table_name(&self) -> &str {
        &self.table_name
    }
}

/// Read write scenario - SELECT + UPDATE
pub struct ReadWrite {
    table_name: String,
    rows: usize,
}

impl ReadWrite {
    pub fn new(_tables: usize, rows: usize) -> Self {
        Self {
            table_name: "sbtest1".to_string(),
            rows,
        }
    }
}

impl Scenario for ReadWrite {
    fn prepare(&self, storage: &mut StorageEngine, rows: usize) -> Result<(), Box<dyn Error>> {
        for i in 1..=rows.min(100) {
            let values = vec![
                Value::Int64(i as i64),
                Value::Int64(i as i64),
                Value::VarChar(format!("data_{}", i)),
                Value::VarChar(format!("pad_{}", i)),
            ];
            storage.insert(&self.table_name, values)?;
        }
        Ok(())
    }

    fn execute(
        &self,
        storage: &mut StorageEngine,
        rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>> {
        // Read
        let _tuples = storage.scan(&self.table_name, None)?;

        // Update
        let id = rng.gen_range(1..=self.rows.min(100)) as i64;
        let values = vec![
            Value::Int64(id),
            Value::Int64(id + 1),
            Value::VarChar("updated".to_string()),
            Value::VarChar("updated".to_string()),
        ];
        // Note: StorageEngine doesn't have direct row update by RowId
        // We need to scan to find the row
        let tuples = storage.scan(&self.table_name, None)?;
        for (idx, tuple) in tuples.iter().enumerate() {
            if let Some(Value::Int64(tid)) = tuple.get(0) {
                if *tid == id {
                    let row_id = RowId::new(
                        // Simplified - in real impl we'd track row_ids properly
                        idx as u64,
                        idx,
                    );
                    let _ = storage.update(&self.table_name, row_id, values.clone());
                    break;
                }
            }
        }
        Ok(())
    }

    fn table_name(&self) -> &str {
        &self.table_name
    }
}

/// Write only scenario - INSERT + UPDATE
pub struct WriteOnly {
    table_name: String,
    rows: usize,
    next_id: AtomicU64,
}

impl WriteOnly {
    pub fn new(_tables: usize, rows: usize) -> Self {
        Self {
            table_name: "sbtest1".to_string(),
            rows,
            next_id: AtomicU64::new((rows.min(100) + 1) as u64),
        }
    }
}

impl Scenario for WriteOnly {
    fn prepare(&self, storage: &mut StorageEngine, rows: usize) -> Result<(), Box<dyn Error>> {
        for i in 1..=rows.min(100) {
            let values = vec![
                Value::Int64(i as i64),
                Value::Int64(i as i64),
                Value::VarChar(format!("data_{}", i)),
                Value::VarChar(format!("pad_{}", i)),
            ];
            storage.insert(&self.table_name, values)?;
        }
        Ok(())
    }

    fn execute(
        &self,
        storage: &mut StorageEngine,
        rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>> {
        // Insert
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let k = rng.r#gen::<i64>();
        let values = vec![
            Value::Int64(id as i64),
            Value::Int64(k),
            Value::VarChar("test".to_string()),
            Value::VarChar("pad".to_string()),
        ];
        storage.insert(&self.table_name, values)?;

        // Update
        let update_id = rng.gen_range(1..=self.rows.min(100) as i64);
        let tuples = storage.scan(&self.table_name, None)?;
        for (idx, tuple) in tuples.iter().enumerate() {
            if let Some(Value::Int64(tid)) = tuple.get(0) {
                if *tid == update_id {
                    let row_id = RowId::new(idx as u64, idx);
                    let _ = storage.update(
                        &self.table_name,
                        row_id,
                        vec![
                            Value::Int64(update_id),
                            Value::Int64(update_id + 1),
                            Value::VarChar("updated".to_string()),
                            Value::VarChar("updated".to_string()),
                        ],
                    );
                    break;
                }
            }
        }
        Ok(())
    }

    fn table_name(&self) -> &str {
        &self.table_name
    }
}

/// Update index scenario - UPDATE indexed column
pub struct UpdateIndex {
    table_name: String,
    rows: usize,
}

impl UpdateIndex {
    pub fn new(_tables: usize, rows: usize) -> Self {
        Self {
            table_name: "sbtest1".to_string(),
            rows,
        }
    }
}

impl Scenario for UpdateIndex {
    fn prepare(&self, storage: &mut StorageEngine, rows: usize) -> Result<(), Box<dyn Error>> {
        for i in 1..=rows.min(100) {
            let values = vec![
                Value::Int64(i as i64),
                Value::Int64(i as i64),
                Value::VarChar(format!("data_{}", i)),
                Value::VarChar(format!("pad_{}", i)),
            ];
            storage.insert(&self.table_name, values)?;
        }
        Ok(())
    }

    fn execute(
        &self,
        storage: &mut StorageEngine,
        rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>> {
        let id = rng.gen_range(1..=self.rows.min(100)) as i64;
        let k = rng.r#gen::<i64>();

        let tuples = storage.scan(&self.table_name, None)?;
        for (idx, tuple) in tuples.iter().enumerate() {
            if let Some(Value::Int64(tid)) = tuple.get(0) {
                if *tid == id {
                    let row_id = RowId::new(idx as u64, idx);
                    storage.update(
                        &self.table_name,
                        row_id,
                        vec![
                            Value::Int64(id),
                            Value::Int64(k),
                            Value::VarChar("updated".to_string()),
                            Value::VarChar("updated".to_string()),
                        ],
                    )?;
                    break;
                }
            }
        }
        Ok(())
    }

    fn table_name(&self) -> &str {
        &self.table_name
    }
}

/// Update non-index scenario - UPDATE non-indexed column
pub struct UpdateNonIndex {
    table_name: String,
    rows: usize,
}

impl UpdateNonIndex {
    pub fn new(_tables: usize, rows: usize) -> Self {
        Self {
            table_name: "sbtest1".to_string(),
            rows,
        }
    }
}

impl Scenario for UpdateNonIndex {
    fn prepare(&self, storage: &mut StorageEngine, rows: usize) -> Result<(), Box<dyn Error>> {
        for i in 1..=rows.min(100) {
            let values = vec![
                Value::Int64(i as i64),
                Value::Int64(i as i64),
                Value::VarChar(format!("data_{}", i)),
                Value::VarChar(format!("pad_{}", i)),
            ];
            storage.insert(&self.table_name, values)?;
        }
        Ok(())
    }

    fn execute(
        &self,
        storage: &mut StorageEngine,
        rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>> {
        let id = rng.gen_range(1..=self.rows.min(100)) as i64;

        let tuples = storage.scan(&self.table_name, None)?;
        for (idx, tuple) in tuples.iter().enumerate() {
            if let Some(Value::Int64(tid)) = tuple.get(0) {
                if *tid == id {
                    let row_id = RowId::new(idx as u64, idx);
                    storage.update(
                        &self.table_name,
                        row_id,
                        vec![
                            Value::Int64(id),
                            Value::Int64(id),
                            Value::VarChar("updated".to_string()),
                            Value::VarChar("updated".to_string()),
                        ],
                    )?;
                    break;
                }
            }
        }
        Ok(())
    }

    fn table_name(&self) -> &str {
        &self.table_name
    }
}

/// Insert scenario - single row insert
pub struct Insert {
    table_name: String,
    next_id: AtomicU64,
}

impl Insert {
    pub fn new(_tables: usize, rows: usize) -> Self {
        Self {
            table_name: "sbtest1".to_string(),
            next_id: AtomicU64::new((rows.min(100) + 1) as u64),
        }
    }
}

impl Scenario for Insert {
    fn prepare(&self, storage: &mut StorageEngine, rows: usize) -> Result<(), Box<dyn Error>> {
        for i in 1..=rows.min(100) {
            let values = vec![
                Value::Int64(i as i64),
                Value::Int64(i as i64),
                Value::VarChar(format!("data_{}", i)),
                Value::VarChar(format!("pad_{}", i)),
            ];
            storage.insert(&self.table_name, values)?;
        }
        Ok(())
    }

    fn execute(
        &self,
        storage: &mut StorageEngine,
        rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let k = rng.r#gen::<i64>();
        let values = vec![
            Value::Int64(id as i64),
            Value::Int64(k),
            Value::VarChar("test data".to_string()),
            Value::VarChar("padding".to_string()),
        ];
        storage.insert(&self.table_name, values)?;
        Ok(())
    }

    fn table_name(&self) -> &str {
        &self.table_name
    }
}

/// Delete scenario - single row delete
pub struct Delete {
    table_name: String,
    rows: usize,
    next_delete_id: AtomicU64,
}

impl Delete {
    pub fn new(_tables: usize, rows: usize) -> Self {
        Self {
            table_name: "sbtest1".to_string(),
            rows,
            next_delete_id: AtomicU64::new(1),
        }
    }
}

impl Scenario for Delete {
    fn prepare(&self, storage: &mut StorageEngine, rows: usize) -> Result<(), Box<dyn Error>> {
        for i in 1..=rows.min(100) {
            let values = vec![
                Value::Int64(i as i64),
                Value::Int64(i as i64),
                Value::VarChar(format!("data_{}", i)),
                Value::VarChar(format!("pad_{}", i)),
            ];
            storage.insert(&self.table_name, values)?;
        }
        Ok(())
    }

    fn execute(
        &self,
        storage: &mut StorageEngine,
        _rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>> {
        let id = self.next_delete_id.fetch_add(1, Ordering::Relaxed);
        if id <= self.rows.min(100) as u64 {
            // Delete requires scanning and finding the row
            let tuples = storage.scan(&self.table_name, None)?;
            for (idx, tuple) in tuples.iter().enumerate() {
                if let Some(Value::Int64(tid)) = tuple.get(0) {
                    if *tid == id as i64 {
                        let row_id = RowId::new(idx as u64, idx);
                        storage.delete(&self.table_name, row_id)?;
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    fn table_name(&self) -> &str {
        &self.table_name
    }
}

/// Bulk insert scenario - batch insert
pub struct BulkInsert {
    table_name: String,
    batch_size: usize,
    next_id: AtomicU64,
}

impl BulkInsert {
    pub fn new(_tables: usize, rows: usize) -> Self {
        Self {
            table_name: "sbtest1".to_string(),
            batch_size: 100,
            next_id: AtomicU64::new((rows.min(100) + 1) as u64),
        }
    }
}

impl Scenario for BulkInsert {
    fn prepare(&self, storage: &mut StorageEngine, rows: usize) -> Result<(), Box<dyn Error>> {
        for i in 1..=rows.min(100) {
            let values = vec![
                Value::Int64(i as i64),
                Value::Int64(i as i64),
                Value::VarChar(format!("data_{}", i)),
                Value::VarChar(format!("pad_{}", i)),
            ];
            storage.insert(&self.table_name, values)?;
        }
        Ok(())
    }

    fn execute(
        &self,
        storage: &mut StorageEngine,
        rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>> {
        let start_id = self
            .next_id
            .fetch_add(self.batch_size as u64, Ordering::Relaxed);

        for i in 0..self.batch_size {
            let id = start_id + i as u64;
        let k = rng.r#gen::<i64>();
            let values = vec![
                Value::Int64(id as i64),
                Value::Int64(k),
                Value::VarChar("data".to_string()),
                Value::VarChar("pad".to_string()),
            ];
            storage.insert(&self.table_name, values)?;
        }
        Ok(())
    }

    fn table_name(&self) -> &str {
        &self.table_name
    }
}
