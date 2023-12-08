use std::fs::OpenOptions;
use std::io::{Write, BufReader, BufRead, Error};

pub fn update_global_identity_name(new_name: &str) -> Result<(), Error> {
    let file_path = ".secret";
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(file_path)?;

    let reader = BufReader::new(&file);

    let mut lines: Vec<String> = reader.lines()
        .filter_map(|result| result.ok())
        .collect();

    let mut found = false;
    for line in &mut lines {
        if line.starts_with("GLOBAL_IDENTITY_NAME=") {
            *line = format!("GLOBAL_IDENTITY_NAME={}", new_name);
            found = true;
        }
    }

    if !found {
        lines.push(format!("GLOBAL_IDENTITY_NAME={}", new_name));
    }

    // Truncate the file and write the updated content
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(file_path)?;

    for line in lines {
        writeln!(file, "{}", line)?;
    }

    Ok(())
}
