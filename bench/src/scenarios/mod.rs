//! Benchmark scenarios module

use aistore::executor::Executor;
use rand::Rng;
use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};

/// Scenario trait - defines a benchmark scenario
pub trait Scenario: Send + Sync {
    /// Prepare scenario (create tables, etc.)
    fn prepare(&self, executor: &mut Executor) -> Result<(), Box<dyn Error>>;

    /// Execute one iteration of the scenario
    fn execute(
        &self,
        executor: &mut Executor,
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
    fn prepare(&self, executor: &mut Executor) -> Result<(), Box<dyn Error>> {
        executor.execute(&format!(
            "CREATE TABLE {} (id INT64, k INT64, c VARCHAR(100), pad VARCHAR(60))",
            self.table_name
        ))?;
        Ok(())
    }

    fn execute(
        &self,
        executor: &mut Executor,
        rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>> {
        let id = rng.gen_range(1..=self.rows) as i64;
        executor.execute(&format!(
            "SELECT * FROM {} WHERE id = {}",
            self.table_name, id
        ))?;
        Ok(())
    }

    fn table_name(&self) -> &str {
        &self.table_name
    }
}

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
    fn prepare(&self, executor: &mut Executor) -> Result<(), Box<dyn Error>> {
        executor.execute(&format!(
            "CREATE TABLE {} (id INT64, k INT64, c VARCHAR(100), pad VARCHAR(60))",
            self.table_name
        ))?;
        Ok(())
    }

    fn execute(
        &self,
        executor: &mut Executor,
        _rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>> {
        executor.execute(&format!("SELECT * FROM {}", self.table_name))?;
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
    fn prepare(&self, executor: &mut Executor) -> Result<(), Box<dyn Error>> {
        executor.execute(&format!(
            "CREATE TABLE {} (id INT64, k INT64, c VARCHAR(100), pad VARCHAR(60))",
            self.table_name
        ))?;
        Ok(())
    }

    fn execute(
        &self,
        executor: &mut Executor,
        rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>> {
        executor.execute(&format!("SELECT * FROM {}", self.table_name))?;
        let id = rng.gen_range(1..=self.rows) as i64;
        executor.execute(&format!(
            "UPDATE {} SET k = k + 1 WHERE id = {}",
            self.table_name, id
        ))?;
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
            next_id: AtomicU64::new((rows + 1) as u64),
        }
    }
}

impl Scenario for WriteOnly {
    fn prepare(&self, executor: &mut Executor) -> Result<(), Box<dyn Error>> {
        executor.execute(&format!(
            "CREATE TABLE {} (id INT64, k INT64, c VARCHAR(100), pad VARCHAR(60))",
            self.table_name
        ))?;
        Ok(())
    }

    fn execute(
        &self,
        executor: &mut Executor,
        rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let k = rng.r#gen::<i64>();
        executor.execute(&format!(
            "INSERT INTO {} VALUES ({}, {}, 'test', 'pad')",
            self.table_name, id, k
        ))?;
        let update_id = rng.gen_range(1..=self.rows as i64);
        executor.execute(&format!(
            "UPDATE {} SET k = k + 1 WHERE id = {}",
            self.table_name, update_id
        ))?;
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
    fn prepare(&self, executor: &mut Executor) -> Result<(), Box<dyn Error>> {
        executor.execute(&format!(
            "CREATE TABLE {} (id INT64, k INT64, c VARCHAR(100), pad VARCHAR(60))",
            self.table_name
        ))?;
        Ok(())
    }

    fn execute(
        &self,
        executor: &mut Executor,
        rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>> {
        let id = rng.gen_range(1..=self.rows) as i64;
        let k = rng.r#gen::<i64>();
        executor.execute(&format!(
            "UPDATE {} SET k = {} WHERE id = {}",
            self.table_name, k, id
        ))?;
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
    fn prepare(&self, executor: &mut Executor) -> Result<(), Box<dyn Error>> {
        executor.execute(&format!(
            "CREATE TABLE {} (id INT64, k INT64, c VARCHAR(100), pad VARCHAR(60))",
            self.table_name
        ))?;
        Ok(())
    }

    fn execute(
        &self,
        executor: &mut Executor,
        rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>> {
        let id = rng.gen_range(1..=self.rows) as i64;
        executor.execute(&format!(
            "UPDATE {} SET pad = 'updated' WHERE id = {}",
            self.table_name, id
        ))?;
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
            next_id: AtomicU64::new((rows + 1) as u64),
        }
    }
}

impl Scenario for Insert {
    fn prepare(&self, executor: &mut Executor) -> Result<(), Box<dyn Error>> {
        executor.execute(&format!(
            "CREATE TABLE {} (id INT64, k INT64, c VARCHAR(100), pad VARCHAR(60))",
            self.table_name
        ))?;
        Ok(())
    }

    fn execute(
        &self,
        executor: &mut Executor,
        rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let k = rng.r#gen::<i64>();
        executor.execute(&format!(
            "INSERT INTO {} VALUES ({}, {}, 'test data', 'padding')",
            self.table_name, id, k
        ))?;
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
    fn prepare(&self, executor: &mut Executor) -> Result<(), Box<dyn Error>> {
        executor.execute(&format!(
            "CREATE TABLE {} (id INT64, k INT64, c VARCHAR(100), pad VARCHAR(60))",
            self.table_name
        ))?;
        Ok(())
    }

    fn execute(
        &self,
        executor: &mut Executor,
        _rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>> {
        let id = self.next_delete_id.fetch_add(1, Ordering::Relaxed);
        if id <= self.rows as u64 {
            executor.execute(&format!(
                "DELETE FROM {} WHERE id = {}",
                self.table_name, id
            ))?;
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
            next_id: AtomicU64::new((rows + 1) as u64),
        }
    }
}

impl Scenario for BulkInsert {
    fn prepare(&self, executor: &mut Executor) -> Result<(), Box<dyn Error>> {
        executor.execute(&format!(
            "CREATE TABLE {} (id INT64, k INT64, c VARCHAR(100), pad VARCHAR(60))",
            self.table_name
        ))?;
        Ok(())
    }

    fn execute(
        &self,
        executor: &mut Executor,
        rng: &mut rand::rngs::StdRng,
    ) -> Result<(), Box<dyn Error>> {
        let start_id = self
            .next_id
            .fetch_add(self.batch_size as u64, Ordering::Relaxed);

        let mut values = String::new();
        for i in 0..self.batch_size {
            let id = start_id + i as u64;
            let k = rng.r#gen::<i64>();
            if i > 0 {
                values.push_str(", ");
            }
            values.push_str(&format!("({}, {}, 'data', 'pad')", id, k));
        }

        executor.execute(&format!(
            "INSERT INTO {} VALUES {}",
            self.table_name, values
        ))?;

        Ok(())
    }

    fn table_name(&self) -> &str {
        &self.table_name
    }
}
