use base64ct::{Base64Url, Encoding};
use blake2::{Blake2s256, Digest};
use serde::Serialize;
use std::fmt::Debug;
use thiserror::Error;

use crate::files::write_file_contents;
use crate::parsing::wrap_html_as_template;
use crate::{BuildConfig, ContentFrontmatter};

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("Couldn't write {name}")]
    WritePermalinkError {
        source: std::io::Error,
        name: String,
    },

    #[error("Couldn't create templating instance")]
    CreateTeraInstanceError { source: tera::Error },

    #[error("Couldn't create template context from data: {data}")]
    CreateTeraContextError { source: tera::Error, data: String },

    #[error("Couldn't extend template context for render: '{render_name}'")]
    ExtendTeraContextError {
        source: tera::Error,
        render_name: String,
    },

    #[error("Template error during render")]
    RenderError { source: tera::Error },
}

pub(crate) struct Renderer<'a> {
    pub template_engine: tera::Tera,
    pub base_context: tera::Context,
    pub build_config: &'a BuildConfig,
}

pub(crate) struct RenderPassDescriptor {
    pub render_name: String,
    pub frontmatter: Option<ContentFrontmatter>,
    pub html: String,
}

pub(crate) struct Render {
    pub pass_descriptor: RenderPassDescriptor,
    pub output: String,
}

impl<'a> Renderer<'a> {
    pub(crate) fn new<T: Serialize + Debug>(
        build_config: &'a BuildConfig,
        context_data: T,
    ) -> Result<Renderer<'a>, Error> {
        if build_config.debug {
            println!("Loading templates from '{}'", &build_config.templates_glob);
            println!("");
        }
        let template_engine = tera::Tera::new(&build_config.templates_glob)
            .map_err(|e| Error::CreateTeraInstanceError { source: e })?;
        if build_config.debug {
            println!("Loaded templates:");
            for name in template_engine.get_template_names() {
                println!("  - \"{}\"", name)
            }
            println!("");
        }

        let base_context = tera::Context::from_serialize(&context_data).map_err(|e| {
            Error::CreateTeraContextError {
                source: e,
                data: format!("{:?}", context_data),
            }
        })?;
        if build_config.debug {
            println!("Loaded base context:");
            let json = base_context.clone().into_json();
            for value in json.as_object().unwrap() {
                println!("  - {} = {}", value.0, value.1);
            }
            println!("");
        }

        Ok(Renderer {
            template_engine,
            base_context,
            build_config,
        })
    }
    pub(crate) fn render(
        &mut self,
        pass_descriptor: RenderPassDescriptor,
    ) -> Result<Render, Error> {
        if self.build_config.debug {
            println!("Rendering: {}", &pass_descriptor.render_name);
        }
        let render_context = match &pass_descriptor.frontmatter {
            Some(frontmatter) => {
                let local_context = tera::Context::from_serialize(frontmatter).map_err(|e| {
                    Error::ExtendTeraContextError {
                        source: e,
                        render_name: pass_descriptor.render_name.clone(),
                    }
                })?;
                let mut render_context = self.base_context.clone();
                render_context.extend(local_context);

                render_context
            }
            None => self.base_context.clone(),
        };
        let base_template_name = match render_context.get("content_template") {
            Some(tera::Value::String(content_template)) => content_template,
            _ => "",
        };
        let template = wrap_html_as_template(&pass_descriptor.html, &base_template_name);
        self.template_engine
            .add_raw_template(&pass_descriptor.render_name, &template)
            .map_err(|e| Error::RenderError { source: e })?;
        let output = self
            .template_engine
            .render(&pass_descriptor.render_name, &render_context)
            .map_err(|e| Error::RenderError { source: e })?;

        Ok(Render {
            pass_descriptor,
            output,
        })
    }
}

pub(crate) fn write_page_to_permalink(
    render: &Render,
    path: impl AsRef<std::path::Path>,
    log: bool,
) -> Result<(), Error> {
    let file_name = format!(
        "{hash}.html",
        hash = Base64Url::encode_string(&Blake2s256::digest(&render.output))
    );
    let permalink_path = path.as_ref().join(file_name);

    write_file_contents(&render.output, &permalink_path).map_err(|e| {
        Error::WritePermalinkError {
            source: e,
            name: render.pass_descriptor.render_name.clone(),
        }
    })?;
    if log {
        println!(
            "Wrote render '{}' to {:?}",
            &render.pass_descriptor.render_name, &permalink_path
        );
    }

    Ok(())
}
