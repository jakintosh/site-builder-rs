use crate::files::{get_relative_path_string, write_file_contents, Error as FilesError};
use crate::parsing::{wrap_content_in_template, ContentContext};
use crate::{BuildConfig, SiteConfig};
use base64ct::{Base64Url, Encoding};
use blake2s_simd::Params;
use std::fmt::Debug;
use thiserror::Error;

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

    #[error("Couldn't determine base template for render '{name}'")]
    AmbiguousTemplateError { name: String },

    #[error("Couldn't determine a destination for render '{name}'")]
    AmbiguousDestinationError { source: FilesError, name: String },

    #[error("Template engine error during render")]
    RenderError { source: tera::Error },
}

pub(crate) struct Renderer<'a> {
    pub template_engine: tera::Tera,
    pub base_context: tera::Context,
    pub build_config: &'a BuildConfig,
    pub site_config: &'a SiteConfig,
}

pub(crate) enum RenderDestination {
    Explicit { path: String, filename: String },
    Permalink,
}

pub(crate) struct RenderPassDescriptor {
    pub render_name: String,
    pub content_context: Option<ContentContext>,
    pub html: String,
    pub destination: RenderDestination,
}

pub(crate) struct Render {
    pub pass_descriptor: RenderPassDescriptor,
    pub output: String,
}

impl<'a> Renderer<'a> {
    pub(crate) fn new(
        build_config: &'a BuildConfig,
        site_config: &'a SiteConfig,
    ) -> Result<Renderer<'a>, Error> {
        let log = build_config.debug;

        if log {
            println!("Loading templates from '{}'", &build_config.templates_glob);
            println!("");
        }
        let template_engine = tera::Tera::new(&build_config.templates_glob)
            .map_err(|e| Error::CreateTeraInstanceError { source: e })?;
        if log {
            println!("Loaded templates:");
            for name in template_engine.get_template_names() {
                println!("  - \"{}\"", name)
            }
            println!("");
        }

        let base_context = tera::Context::from_serialize(&site_config.context).map_err(|e| {
            Error::CreateTeraContextError {
                source: e,
                data: format!("{:?}", &site_config.context),
            }
        })?;
        if log {
            println!("Loaded base context:");
            for value in base_context.clone().into_json().as_object().unwrap() {
                println!("  - {} = {}", value.0, value.1);
            }
            println!("");
        }

        Ok(Renderer {
            template_engine,
            base_context,
            build_config,
            site_config,
        })
    }
    pub(crate) fn render(&mut self, pass_descriptor: RenderPassDescriptor) -> Result<(), Error> {
        let log = self.build_config.debug;

        // figure out destination and base url
        let destination = match &pass_descriptor.destination {
            RenderDestination::Explicit { path, .. } => path,
            RenderDestination::Permalink => &self.build_config.output_perma_dir_path,
        };
        let base_url = get_relative_path_string(&self.build_config.output_dir_path, destination)
            .map_err(|e| Error::AmbiguousDestinationError {
                source: e,
                name: pass_descriptor.render_name.clone(),
            })?;

        // create render context
        let mut base_context = self.base_context.clone();
        base_context.insert("base_url", &base_url);
        let render_context = match &pass_descriptor.content_context {
            Some(context) => {
                context.extend_context(&mut base_context);

                base_context
            }
            None => base_context,
        };

        // get base template name
        let content_type = match render_context.get("content_type") {
            Some(tera::Value::String(content_type)) => content_type,
            _ => &self.site_config.context.content_type,
        };
        let base_template_name = match self.site_config.content_types.get(content_type) {
            Some(base_template) => &base_template.content_template,
            None => {
                return Err(Error::AmbiguousTemplateError {
                    name: pass_descriptor.render_name.clone(),
                })
            }
        };

        if log {
            println!(
                "Rendering '{}' from base template '{}'",
                &pass_descriptor.render_name, &base_template_name
            );
            println!("  Rendered with context:");
            for value in render_context.clone().into_json().as_object().unwrap() {
                println!("    - {} = {}", value.0, value.1);
            }
        }

        let template = wrap_content_in_template(&pass_descriptor.html, &base_template_name);
        self.template_engine
            .add_raw_template(&pass_descriptor.render_name, &template)
            .map_err(|e| Error::RenderError { source: e })?;
        let output = self
            .template_engine
            .render(&pass_descriptor.render_name, &render_context)
            .map_err(|e| Error::RenderError { source: e })?;

        // export
        export(
            Render {
                pass_descriptor,
                output,
            },
            self.build_config,
        )?;

        Ok(())
    }
}

fn export(render: Render, build_config: &BuildConfig) -> Result<(), Error> {
    let (filename, path) = match render.pass_descriptor.destination {
        RenderDestination::Explicit { path, filename } => (filename, path),
        RenderDestination::Permalink => {
            let hash = Params::new()
                .hash_length(12)
                .hash(&render.output.as_bytes());
            let hash_string = Base64Url::encode_string(hash.as_bytes());
            let filename = format!("{}.html", hash_string);
            let path = build_config.output_perma_dir_path.clone();
            (filename, path)
        }
    };
    let path = format!("{}/{}", path, filename);
    write_file_contents(&render.output, &path).map_err(|e| Error::WritePermalinkError {
        source: e,
        name: render.pass_descriptor.render_name.clone(),
    })?;

    if build_config.debug {
        println!(
            "Exported rendered '{}' to {:?}\n",
            &render.pass_descriptor.render_name, &path
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {}
}
