use anyhow::{Context, Result};
use clap::{command, Parser};
use ignore::WalkBuilder;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Specify a relative path to include only files below that path
    #[arg(short = 'P')]
    path: Option<PathBuf>,

    /// Specify a filename to save the prompt
    #[arg(short = 'F')]
    output_file: Option<PathBuf>,

    /// Specify pattern(s) to match filenames
    #[arg(short = 'M')]
    match_pattern: Option<String>,

    /// Only include files tracked by git
    #[arg(short = 'g')]
    git_only: bool,

    /// Respect case sensitivity in pattern matching
    #[arg(short = 'c')]
    case_sensitive: bool,
}

fn build_tree(entries: &[PathBuf]) -> String {
    let mut tree = String::from(".\n");
    for entry in entries {
        if let Some(path) = entry.to_str() {
            tree.push_str(&format!("├── {}\n", path));
        }
    }
    tree
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("Starting file scan...");

    // Determine the root path
    let root_path = args.path.unwrap_or_else(|| PathBuf::from("."));
    let root_path = fs::canonicalize(&root_path)?;
    println!("Scanning directory: {}", root_path.display());

    // Build the walker with ignore files
    let mut walker = WalkBuilder::new(&root_path);

    // Add .gitignore patterns
    if Path::new(".gitignore").exists() {
        println!("Found .gitignore - applying ignore patterns");
        walker.add_ignore(Path::new(".gitignore"));
    }

    // Add .preludeignore patterns
    if Path::new(".preludeignore").exists() {
        println!("Found .preludeignore - applying ignore patterns");
        walker.add_ignore(Path::new(".preludeignore"));
    }

    // Configure git integration
    if args.git_only {
        println!("Git-only mode enabled - only including tracked files");
        walker.git_ignore(true);
        walker.git_global(true);
        walker.git_exclude(true);
    }

    // Set case sensitivity
    walker.ignore_case_insensitive(!args.case_sensitive);
    if args.case_sensitive {
        println!("Case-sensitive matching enabled");
    }

    println!("\nCollecting files...");

    // Collect all valid files
    let mut files: Vec<PathBuf> = Vec::new();
    let mut ignored_files: Vec<PathBuf> = Vec::new();

    // First, get all entries without ignoring any
    let all_entries = WalkBuilder::new(&root_path)
        .git_ignore(args.git_only)
        .git_global(args.git_only)
        .git_exclude(args.git_only)
        .ignore_case_insensitive(!args.case_sensitive)
        .build()
        .map(|r| r.map_err(anyhow::Error::from))
        .collect::<Result<Vec<_>>>()?;

    // Then get entries with ignoring enabled
    let filtered_entries = walker.build()
        .map(|r| r.map_err(anyhow::Error::from))
        .collect::<Result<Vec<_>>>()?;

    // Find ignored files by comparing the two sets of entries
    'outer: for entry in all_entries {
        let path = entry.path().strip_prefix(&root_path).unwrap().to_path_buf();
        for filtered_entry in &filtered_entries {
            let filtered_path = filtered_entry.path().strip_prefix(&root_path).unwrap().to_path_buf();
            if path == filtered_path {
                continue 'outer;
            }
        }
        ignored_files.push(path);
    }

    // Now collect the filtered files
    for entry in filtered_entries {
        if entry.file_type().map_or(false, |ft| ft.is_file()) {
            if let Ok(path) = entry.path().strip_prefix(&root_path) {
                println!("Reading: {}", path.display());
                files.push(path.to_path_buf());
            }
        }
    }

    // Print ignored files
    /*if !ignored_files.is_empty() {
        println!("\nIgnored files:");
        for file in &ignored_files {
            println!("├── {}", file.display());
        }
    }*/

    println!("\nFound {} files", files.len());
    println!("Sorting file list...");

    files.sort();

    println!("Building file tree...");
    let tree = build_tree(&files);

    println!("Reading file contents...");

    let mut concatenated = String::new();
    for file in &files {
        let full_path = root_path.join(file);
        match fs::read_to_string(&full_path) {
            Ok(content) => {
                println!("Processing: {}", file.display());
                concatenated.push_str(&format!(
                    "\n\n--- File: {} ---\n\n{}",
                    file.display(),
                    content
                ));
            }
            Err(err) => eprintln!("Error reading {}: {}", file.display(), err),
        }
    }

    println!("\nBuilding final prompt...");

    let prompt = format!(
        "I want you to help me fix some issues with my code. \n
         I have attached the code and file structure.\n\n
         File Tree:\n{}\n
         Concatenated Files:{}",
        tree, concatenated
    );

    if let Some(output_file) = args.output_file {
        println!("Saving to file: {}", output_file.display());
        fs::write(&output_file, &prompt).context("Failed to write output file")?;
        println!("Successfully saved prompt to {}", output_file.display());
    }
    println!("\nProcess completed successfully!");

    Ok(())
}

