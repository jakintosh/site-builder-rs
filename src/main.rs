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
use crate::parsing::{parse_blocks_file, parse_json_file, Content, Page, Post, SiteContext};
use crate::rendering::{RenderDestination, Renderer};
use anyhow::{Context, Result};
use clap::Parser;
use rendering::RenderPassDescriptor;
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

    /// Path to config.json file
    #[clap(short, long)]
    config: Option<String>,

    /// Build the site in debug mode
    #[clap(long)]
    debug: bool,
}

struct BuildConfig {
    debug: bool,
    source_dir_path: String,
    output_dir_path: String,
    config_file_path: String,
    content_dir_path: String,
    css_dir_path: String,
    output_perma_dir_path: String,
    content_glob: String,
    components_glob: String,
    templates_glob: String,
}

struct SiteConfig {
    context: SiteContext,
    posts: HashMap<String, Post>,
    pages: HashMap<String, Page>,
}

static DEFAULT_CONFIG_PATH: &str = "config.json";

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
    expect_file(&config_file_path).context("Missing expected config.json file")?;

    let content_dir_path = format!("{src}/content", src = source_dir_path);
    expect_directory(&content_dir_path).context(r"Missing expected {src}/content directory")?;

    let css_dir_path = format!("{src}/css", src = source_dir_path);
    expect_directory(&css_dir_path).context(r"Missing expected {src}/css directory")?;

    let output_perma_dir_path = format!("{out}/permalink", out = output_dir_path);
    ensure_directory(&output_perma_dir_path)
        .context(r"Couldn't create {out}/permalink directory")?;

    let content_glob = format!("{cnt}/**/*.*", cnt = content_dir_path);
    let templates_glob = format!("{src}/templates/**/*.tmpl", src = source_dir_path);
    let components_glob = format!("{src}/components/**/*", src = source_dir_path);

    Ok(BuildConfig {
        debug: args.debug,
        source_dir_path,
        config_file_path,
        output_dir_path,
        content_dir_path,
        css_dir_path,
        output_perma_dir_path,
        content_glob,
        templates_glob,
        components_glob,
    })
}

fn create_site_config(
    path: impl AsRef<std::path::Path>,
    pages: HashMap<String, Page>,
    posts: HashMap<String, Post>,
) -> Result<SiteConfig> {
    let raw_context: SiteContext =
        parse_json_file(path).context("Couldn't load config.json file")?;

    let context = raw_context.clone();

    Ok(SiteConfig {
        context,
        pages,
        posts,
    })
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.debug {
        println!("\n================== Begin Site Builder ==================\n");
    }

    // build config struct
    let build_config = create_build_config(args)
        .context("Failed to create a build configuration from CLI args")?;

    // load all content
    let mut posts: HashMap<String, Post> = HashMap::new();
    let mut pages: HashMap<String, Page> = HashMap::new();
    let content_paths = get_paths_from_glob(&build_config.content_glob)
        .context("Failed to resolve content path glob")?;
    for path in content_paths {
        let content_name = get_stripped_base_path_string(&path, &build_config.content_dir_path)
            .context("Failed to strip content path prefix")?;

        match parse_blocks_file(&path)
            .context(format!("Failed to parse block file: {:?}", &path))?
        {
            Content::Post(post) => {
                posts.insert(content_name, post);
            }
            Content::Page(page) => {
                pages.insert(content_name, page);
            }
        };
    }

    // build site config
    let site_config = create_site_config(&build_config.config_file_path, pages, posts)
        .context("Failed to create a site configuration from config file")?;

    // create renderer
    let mut renderer = Renderer::new(&build_config, &site_config)
        .context("Failed to create a site template renderer")?;

    // build sitemap
    for section in &site_config.context.sections {
        let section_path = format!("{}/{}", build_config.output_dir_path, section.site_path);
        ensure_directory(&section_path).context(format!(
            "Couldn't ensure required sitemap directory '{}'",
            section.site_path,
        ))?;
    }

    // render posts
    for (name, post) in &site_config.posts {
        // describe the render pass
        let desc = RenderPassDescriptor {
            render_name: name.clone(),
            base_template: "post.tmpl",
            context: &post,
            destination: RenderDestination::Explicit {
                directory: format!(
                    "{}/{}",
                    build_config.output_dir_path.clone(),
                    post.metadata.directory.clone()
                ),
                filename: post.metadata.content_name.clone(),
            },
        };

        // render, get export info
        let export = renderer
            .render_content(desc)
            .context(format!("Failed to render '{}'", &name))?;

        // add the exported url to the renderer context
        let site_path = get_stripped_base_path_string(export.path, &build_config.output_dir_path)
            .context(format!(
            "couldn't get site-scoped path from export.path for '{}'",
            export.render_name
        ))?;
        renderer.register_post_url(&export.render_name, site_path);
    }

    // render sections
    for section in &site_config.context.sections {
        // build the directory for this section
        let section_path = format!("{}/{}", build_config.output_dir_path, section.site_path);
        ensure_directory(&section_path).context(format!(
            "Couldn't ensure required sitemap directory '{}'",
            section.site_path,
        ))?;
        let desc = RenderPassDescriptor {
            render_name: section.index_content.clone(),
            base_template: "content.tmpl",
            destination: RenderDestination::SectionIndex {
                directory: section_path,
            },
            context: site_config
                .pages
                .get(&section.index_content)
                .expect(&format!(
                    "Missing index page for section '{}'",
                    section.name
                )),
        };
        renderer
            .render_content(desc)
            .context(format!("Failed to render section '{}'", &section.name))?;
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
