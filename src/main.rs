/// site-builder-rs
///
/// a static site builder for me, jakintosh
///
/// to use:
/// `$ site-builder -s {$SOURCE_FILE_DIRECTORY} -d {$OUTPUT_DIRECTORY}`
/// `$ site-builder --help`
///
mod files;
mod parsing;
mod rendering;

use crate::files::*;
use crate::parsing::{parse_supported_file, parse_toml_file, SiteContentType, SiteContext};
use crate::rendering::{RenderDestination, RenderPassDescriptor, Renderer};
use anyhow::{Context, Result};
use clap::Parser;
use rendering::Export;
use std::collections::HashMap;

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

struct BuildConfig {
    debug: bool,
    source_dir_path: String,
    config_file_path: String,
    output_dir_path: String,
    content_dir_path: String,
    css_dir_path: String,
    output_perma_dir_path: String,
    templates_glob: String,
    components_glob: String,
    content_md_glob: String,
    content_html_glob: String,
}

struct SiteConfig {
    content_types: HashMap<String, SiteContentType>,
    context: SiteContext,
}

static DEFAULT_CONFIG_PATH: &str = "config.toml";

fn create_build_config(args: Args) -> Result<BuildConfig> {
    let source_dir_path = args.source;
    expect_directory(&source_dir_path).context(r"Missing expected {source} directory")?;

    let output_dir_path = args.destination;
    ensure_directory(&output_dir_path).context(r"Couldn't create {output} directory")?;

    let config_file_path = match args.config {
        Some(user_given_config_path) => user_given_config_path.to_owned(),
        None => format!(
            "{src}/{cfg}",
            src = source_dir_path,
            cfg = DEFAULT_CONFIG_PATH
        ),
    };
    expect_file(&config_file_path).context("Missing expected config.toml file")?;

    let content_dir_path = format!("{src}/content", src = source_dir_path);
    expect_directory(&content_dir_path).context(r"Missing expected {src}/content directory")?;

    let css_dir_path = format!("{src}/css", src = source_dir_path);
    expect_directory(&css_dir_path).context(r"Missing expected {src}/css directory")?;

    let output_perma_dir_path = format!("{out}/permalink", out = output_dir_path);
    ensure_directory(&output_perma_dir_path)
        .context(r"Couldn't create {out}/permalink directory")?;

    let templates_glob = format!("{src}/templates/**/*.tmpl", src = source_dir_path);
    let components_glob = format!("{src}/components/**/*", src = source_dir_path);
    let content_md_glob = format!("{cnt}/**/*.md", cnt = content_dir_path);
    let content_html_glob = format!("{cnt}/**/*.html", cnt = content_dir_path);

    Ok(BuildConfig {
        debug: args.debug,
        source_dir_path,
        config_file_path,
        output_dir_path,
        content_dir_path,
        css_dir_path,
        output_perma_dir_path,
        templates_glob,
        components_glob,
        content_md_glob,
        content_html_glob,
    })
}

fn create_site_config(path: impl AsRef<std::path::Path>) -> Result<SiteConfig> {
    let raw_context: SiteContext =
        parse_toml_file(path).context("Couldn't load config.toml file")?;

    let context = raw_context.clone();

    let content_types: HashMap<String, SiteContentType> = raw_context
        .content_types
        .into_iter()
        .map(|c| (c.name.clone(), c))
        .collect();

    Ok(SiteConfig {
        content_types,
        context,
    })
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.debug {
        println!("\n================== Begin Site Builder ==================\n");
    }

    // build config structs
    let build_config = create_build_config(args)
        .context("Failed to create a build configuration from CLI args")?;
    let site_config = create_site_config(&build_config.config_file_path)
        .context("Failed to create a site configuration from config file")?;

    // create renderer
    let mut renderer = Renderer::new(&build_config, &site_config)
        .context("Failed to create a site html renderer")?;

    // render all content
    let mut exports: Vec<Export> = Vec::new();
    let md_paths = get_paths_from_glob(&build_config.content_md_glob)
        .context("Failed to resolve markdown glob")?;
    let html_paths = get_paths_from_glob(&build_config.content_html_glob)
        .context("Failed to resolve html glob")?;
    let paths: Vec<_> = vec![md_paths, html_paths].into_iter().flatten().collect();
    for path in paths {
        let render_name = get_stripped_base_path_string(&path, &build_config.content_dir_path)
            .context("Failed to strip content path prefix")?;
        let (context, html) = match parse_supported_file(&path)
            .context("Failed to parse content file for rendering")?
        {
            Some((content, html)) => (content, html),
            None => {
                println!("Didn't render unexpected '/content' file {:?}", path);
                continue;
            }
        };
        let rpd = RenderPassDescriptor {
            render_name: render_name.clone(),
            content_context: context,
            html,
            destination: RenderDestination::Permalink,
        };
        let export = renderer
            .render(rpd)
            .context(format!("Failed to render content '{}'", render_name))?;
        exports.push(export);
    }

    // go through all the exports, and insert each export into the
    // tera::Context depending on its content type

    // process our sections to build a site graph
    for section in &site_config.context.sections {
        if build_config.debug {
            println!("Building section '{}'", section.name);
        }
        let index_content_name = section.index_content.clone();
        let index_content_path = std::path::PathBuf::from(format!(
            "{}/{}",
            &build_config.source_dir_path, &index_content_name
        ));
        // build the directory for this section
        let section_path = format!("{}/{}", build_config.output_dir_path, section.site_path);
        ensure_directory(&section_path).context(format!(
            "Couldn't ensure required sitemap directory '{}'",
            section.site_path,
        ))?;

        // render the index content page
        let (context, html) = match parse_supported_file(index_content_path)
            .context("Failed to load section index file")?
        {
            Some((context, html)) => (context, html),
            None => continue,
        };

        let rpd = RenderPassDescriptor {
            render_name: index_content_name,
            content_context: context,
            html,
            destination: RenderDestination::Explicit {
                path: section_path.clone(),
                filename: String::from("index.html"),
            },
        };
        let _render = renderer.render(rpd)?;
    }

    // copy over css
    let css_out_path = format!("{}/css", &build_config.output_dir_path);
    dircpy::copy_dir_advanced(
        &build_config.css_dir_path,
        &css_out_path,
        true,
        false,
        false,
        vec![],
        vec![],
    )
    .expect("css failed to copy");

    if build_config.debug {
        println!("\n=================== End Site Builder ===================\n");
    }

    Ok(())
}
