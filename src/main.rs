/// site-builder-rs
///
/// a static site builder for me, jakintosh
///
mod files;
mod parsing;
mod rendering;

use anyhow::{Context, Result};
use clap::Parser;
use files::{
    ensure_directory, expect_directory, expect_file, get_paths_from_glob, get_relative_path_string,
};
use parsing::{parse_html_file, parse_markdown_file, parse_toml_file};
use rendering::{write_page_to_permalink, RenderPassDescriptor, Renderer};
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[clap(name = "site-builer")]
#[clap(author = "@jakintosh")]
#[clap(version = "0.1.0")]
#[clap(about = "builds jakintosh.com", long_about = None)]
struct Args {
    /// Directory where content is sourced from
    #[clap(short, long)]
    source: String,

    /// Directory where the site is built to
    #[clap(short, long)]
    destination: String,

    /// Path to config.toml file
    #[clap(short, long)]
    config: Option<String>,

    /// Build the site in debug mode
    #[clap(long)]
    debug: bool,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
pub(crate) struct ContentFrontmatter {
    pub content_type: Option<String>,
    pub content_title: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct SiteConfiguration {
    pub site_title: String,
    pub language_code: String,
    pub content_template: String,
    pub base_url: String,
    pub sections: Vec<SiteSection>,
    pub content_types: Vec<SiteContentType>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct SiteSection {
    pub name: String,
    pub site_path: String,
    pub index_content: String,
    pub priority: u8,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct SiteContentType {
    pub name: String,
    pub content_template: String,
}

#[derive(Clone)]
struct BuildConfig {
    pub debug: bool,
    pub config_file: String,
    pub output_dir: String,
    pub content_dir: String,
    pub css_dir: String,
    pub output_perma_dir: String,
    pub templates_glob: String,
    pub content_md_glob: String,
    pub content_html_glob: String,
}

static DEFAULT_CONFIG_PATH: &str = "config.toml";

fn create_build_config(args: Args) -> Result<BuildConfig> {
    // parse args
    let source_path = args.source;
    let output_path = args.destination;
    let config_path = match args.config {
        Some(user_given_config_path) => user_given_config_path.to_owned(),
        None => format!("{src}/{cfg}", src = source_path, cfg = DEFAULT_CONFIG_PATH),
    };

    // build multi-use path bases
    let content_path = format!("{src}/content", src = source_path);

    // create build config
    let build_config = BuildConfig {
        debug: args.debug,
        config_file: config_path,
        output_dir: output_path.clone(),
        content_dir: content_path.clone(),
        css_dir: format!("{src}/css", src = source_path),
        output_perma_dir: format!("{out}/permalink", out = output_path),
        templates_glob: format!("{src}/templates/**/*.tmpl", src = source_path),
        content_md_glob: format!("{src}/**/*.md", src = content_path),
        content_html_glob: format!("{src}/**/*.html", src = content_path),
    };

    // verify all pieces of the config
    expect_file(&build_config.config_file).context("Missing expected config.toml file")?;
    expect_directory(&build_config.content_dir).context("Missing expected /content directory")?;
    expect_directory(&build_config.css_dir).context("Missing expected /css directory")?;
    ensure_directory(&build_config.output_dir).context("Couldn't create output directory")?;
    ensure_directory(&build_config.output_perma_dir)
        .context("Couldn't create /output/permalink directory")?;

    Ok(build_config)
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.debug {
        println!("\n================== Begin Site Builder ==================\n");
    }

    let build_config = create_build_config(args)
        .context("Failed to create site build configuration from CLI args")?;
    let site_config: SiteConfiguration = parse_toml_file(&build_config.config_file)
        .context("Couldn't parse site 'config.toml' file")?;
    let mut renderer = Renderer::new(&build_config, &site_config)
        .context("Failed to create a site html renderer")?;

    // render all markdown
    for md_file_path in get_paths_from_glob(&build_config.content_md_glob)? {
        let render_name = get_relative_path_string(&md_file_path, &build_config.content_dir)?;
        let (frontmatter, html) = parse_markdown_file(&md_file_path)?;
        let rpd = RenderPassDescriptor {
            render_name: render_name.clone(),
            frontmatter,
            html,
        };
        let render = renderer.render(rpd)?;
        write_page_to_permalink(&render, &build_config.output_perma_dir, build_config.debug)?;
    }

    // render all html
    for html_file_path in get_paths_from_glob(&build_config.content_html_glob)? {
        let render_name = get_relative_path_string(&html_file_path, &build_config.content_dir)?;
        let (frontmatter, html) = parse_html_file(&html_file_path)?;
        let rpd = RenderPassDescriptor {
            render_name,
            frontmatter,
            html,
        };
        let render = renderer.render(rpd)?;
        write_page_to_permalink(&render, &build_config.output_perma_dir, build_config.debug)?;
    }

    // // load all content types
    // let mut content_types: HashMap<String, SiteContentType> = HashMap::new();
    // for content_type in site_config.content_types {
    //     content_types.insert(content_type.name.clone(), content_type);
    // }

    // // process our sections to build a site graph
    // for section in site_config.sections {
    //     let section_path = format!("{}/{}", build_config.output_dir, section.site_path);
    //     ensure_directory(&section_path)?;
    // }

    // copy over css
    let css_out_path = format!("{}/css", &build_config.output_dir);
    dircpy::copy_dir(&build_config.css_dir, &css_out_path).expect("css failed to copy");

    if build_config.debug {
        println!("\n=================== End Site Builder ===================\n");
    }

    Ok(())
}
