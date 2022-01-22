use crate::files::write_file_contents;
use crate::parsing::{wrap_content_in_template, ContentContext};
use crate::{BuildConfig, SiteConfig};
use base64ct::{Base64Url, Encoding};
use blake2::{Blake2s256, Digest};
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

    #[error("Template engine error during render")]
    RenderError { source: tera::Error },
}

pub(crate) struct Renderer<'a> {
    pub template_engine: tera::Tera,
    pub base_context: tera::Context,
    pub build_config: &'a BuildConfig,
    pub site_config: &'a SiteConfig,
}

pub(crate) struct RenderPassDescriptor {
    pub render_name: String,
    pub context: Option<ContentContext>,
    pub html: String,
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
    pub(crate) fn render(
        &mut self,
        pass_descriptor: RenderPassDescriptor,
    ) -> Result<Render, Error> {
        let log = self.build_config.debug;
        let render_context = match &pass_descriptor.context {
            Some(context) => {
                let mut render_context = self.base_context.clone();
                context.extend_context(&mut render_context);

                render_context
            }
            None => self.base_context.clone(),
        };

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
    let hash = Base64Url::encode_string(&Blake2s256::digest(&render.output));
    let file_name = format!("{hash}.html", hash = hash);
    let permalink_path = path.as_ref().join(file_name);
    write_file_contents(&render.output, &permalink_path).map_err(|e| {
        Error::WritePermalinkError {
            source: e,
            name: render.pass_descriptor.render_name.clone(),
        }
    })?;

    if log {
        println!(
            "Exported rendered '{}' to {:?}\n",
            &render.pass_descriptor.render_name, &permalink_path
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {}
}
