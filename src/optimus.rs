use crate::blocks::Blocks;
use crate::files::*;
use anyhow::{Context, Result};
use clap::Parser;
use glob::glob;
use serde::Deserialize;
use serde_json;

#[derive(Parser)]
#[clap(name = "site-buidler")]
#[clap(author = "@jakintosh")]
#[clap(version = "0.1.0")]
#[clap(about = "builds a website", long_about = None)]
struct Args {
    /// Directory for content to be rendered
    #[clap(short, long)]
    content_dir: Option<String>,

    /// Directory for static content to be copied
    #[clap(short, long)]
    static_dir: Option<String>,

    /// Directory where the site is built to
    #[clap(short, long)]
    output_dir: Option<String>,

    /// Path to config.json file
    #[clap(short, long)]
    config: Option<String>,
}

#[derive(Deserialize)]
struct Config {
    pub content_dir: String,
    pub static_dir: String,
    pub output_dir: String,
}

fn get_config(args: Args) -> Result<Config> {
    let config_file = match args.config {
        Some(config_path) => {
            let json = read_file_contents(config_path).context("Couldn't read config file")?;
            serde_json::from_str(&json).context("Json deserialization failure")?
        }
        None => serde_json::Value::Null,
    };

    let content_dir = match args.content_dir {
        Some(c) => c,
        None => config_file
            .get("content")
            .context("Missing required config option: 'content'")?
            .as_str()
            .context("Something")?
            .to_owned(),
    };
    ensure_directory(&content_dir)?;

    let static_dir = match args.static_dir {
        Some(c) => c,
        None => config_file
            .get("static")
            .context("Missing required config option: 'static'")?
            .as_str()
            .context("Something")?
            .to_owned(),
    };
    ensure_directory(&static_dir)?;

    let output_dir = match args.output_dir {
        Some(c) => c,
        None => config_file
            .get("output")
            .context("Missing required config option: 'output'")?
            .as_str()
            .context("Something")?
            .to_owned(),
    };
    ensure_directory(&output_dir)?;

    Ok(Config {
        content_dir,
        static_dir,
        output_dir,
    })
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config = get_config(args).context("Couldn't load valid config")?;

    let content_glob = format!("{}/*", config.content_dir);
    for path in glob(&content_glob)? {
        if let Ok(path) = path {
            let file_contents = read_file_contents(path)?;
            let blocks: Blocks = file_contents.parse()?;
        }
    }

    Ok(())
}
