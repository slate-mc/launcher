use slate_contracts::schema_documents;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const USAGE: &str = "usage: cargo run -p xtask -- <contracts-generate|contracts-check>";

fn main() {
    if let Err(error) = run(std::env::args_os().skip(1)) {
        eprintln!("xtask: {error}");
        std::process::exit(1);
    }
}

fn run(mut arguments: impl Iterator<Item = OsString>) -> Result<(), Box<dyn std::error::Error>> {
    let command = arguments.next().ok_or(USAGE)?;
    if arguments.next().is_some() {
        return Err(USAGE.into());
    }

    let output = workspace_root().join("schemas").join("ipc");
    match command.to_str() {
        Some("contracts-generate") => generate(&output),
        Some("contracts-check") => check(&output),
        _ => Err(USAGE.into()),
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map_or_else(
            || PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            Path::to_path_buf,
        )
}

fn generated_documents() -> Result<Vec<(String, Vec<u8>)>, serde_json::Error> {
    schema_documents()
        .into_iter()
        .map(|(name, schema)| {
            let mut bytes = serde_json::to_vec_pretty(&schema)?;
            bytes.push(b'\n');
            Ok((format!("{name}.schema.json"), bytes))
        })
        .collect()
}

fn generate(output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(output)?;
    for (name, bytes) in generated_documents()? {
        std::fs::write(output.join(name), bytes)?;
    }
    println!("generated IPC schemas in {}", output.display());
    Ok(())
}

fn check(output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut stale = Vec::new();
    for (name, expected) in generated_documents()? {
        let path = output.join(&name);
        match std::fs::read(&path) {
            Ok(actual) if actual == expected => {}
            Ok(_) | Err(_) => stale.push(name),
        }
    }

    if stale.is_empty() {
        println!("IPC schemas are current");
        Ok(())
    } else {
        Err(format!(
            "generated IPC schemas are missing or stale: {}. Run contracts-generate.",
            stale.join(", ")
        )
        .into())
    }
}

#[cfg(test)]
mod tests {
    use super::generated_documents;

    #[test]
    fn generated_schema_documents_are_stable_and_nonempty() -> Result<(), Box<dyn std::error::Error>>
    {
        let documents = generated_documents()?;

        assert_eq!(documents.len(), 85);
        assert!(documents.iter().all(|(_, bytes)| !bytes.is_empty()));
        Ok(())
    }
}
