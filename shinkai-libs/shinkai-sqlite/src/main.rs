use rusqlite::{params, Connection, Result};
use std::time::Instant;

fn main() -> Result<()> {
    /*
    SQL for logs and other stuff:

    - we need to pass where the db will be stored (probably next to rocksdb and lancedb)
    - we need to create the tables if they don't exist
     */
    let conn = Connection::open("example.db")?;

    // Create the first table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS parent (
                  id INTEGER PRIMARY KEY,
                  name TEXT NOT NULL
                  )",
        [],
    )?;

    // Create the second table with a foreign key pointing to the first table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS child (
                  id INTEGER PRIMARY KEY,
                  parent_id INTEGER,
                  name TEXT NOT NULL,
                  FOREIGN KEY(parent_id) REFERENCES parent(id)
                  )",
        [],
    )?;

    // Benchmark for inserting 1,000 records
    let start_insert = Instant::now();
    for i in 1..=1000 {
        conn.execute(
            "INSERT INTO parent (name) VALUES (?1)",
            params![format!("Parent {}", i)],
        )?;
        conn.execute(
            "INSERT INTO child (parent_id, name) VALUES (?1, ?2)",
            params![i, format!("Child {}", i)],
        )?;
    }
    let duration_insert = start_insert.elapsed();
    println!("Time taken to insert 1,000 records: {:?}", duration_insert);

    // Benchmark for reading 1,000 records
    let start_read = Instant::now();
    let mut stmt = conn.prepare("SELECT child.id, child.name, parent.name FROM child JOIN parent ON child.parent_id = parent.id")?;
    let child_iter = stmt.query_map([], |row| {
        Ok((row.get::<_, i32>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
    })?;

    for child in child_iter {
        let (child_id, child_name, parent_name) = child?;
        // println!("Child ID: {}, Child Name: {}, Parent Name: {}", child_id, child_name, parent_name);
    }
    let duration_read = start_read.elapsed();
    println!("Time taken to read 1,000 records: {:?}", duration_read);

    Ok(())
}