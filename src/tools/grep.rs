use crate::tools::{Tool, ToolError};
use async_trait::async_trait;
use grep::{
    regex::RegexMatcher,
    searcher::{BinaryDetection, SearcherBuilder, sinks::UTF8},
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::{error::Error, fs, path::PathBuf};

#[derive(Deserialize, JsonSchema)]
struct GrepArgs {
    #[serde()]
    pattern: String,
    path: String,
}

pub struct Grep {}

impl Grep {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Tool for Grep {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search for a specific string or regular expression within files. Use this tool to find where a variable, function, or class is defined or used across the codebase. You can provide a directory path to narrow down the search. If no path is provided, it searches the entire project."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(GrepArgs)).unwrap()
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String, ToolError> {
        let args: GrepArgs = serde_json::from_value(args).map_err(|e| ToolError(e.to_string()))?;
        let matches = search(&args.pattern, args.path).map_err(|e| ToolError(e.to_string()))?;

        Ok(matches.join("\n"))
    }
}

// taken from https://github.com/BurntSushi/ripgrep/blob/master/crates/grep/examples/simplegrep.rs
fn search(pattern: &str, path: String) -> Result<Vec<String>, Box<dyn Error>> {
    let matcher = RegexMatcher::new_line_matcher(pattern)?;
    let mut searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .line_number(true)
        .build();

    let files: Vec<PathBuf> = fs::read_dir(path)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|entry| entry.is_file())
        .take(200)
        .collect();
    let mut matches = Vec::new();

    for file in files {
        searcher.search_path(
            &matcher,
            &file,
            UTF8(|line_number, line| {
                matches.push(format!(
                    "{}:{}:{}",
                    file.display(),
                    line_number,
                    line.trim_end()
                ));
                Ok(true)
            }),
        )?;
    }

    Ok(matches)
}
