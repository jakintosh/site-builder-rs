use crate::files::{
    ensure_directory, get_relative_path_string, load_component_files, write_file_contents,
    Error as FilesError,
};
use crate::{BuildConfig, SiteConfig};
use base64ct::{Base64Url, Encoding};
use blake2s_simd::Params;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("Couldn't write {name}")]
    WriteExportError { source: FilesError, name: String },

    #[error("Couldn't create templating instance")]
    CreateTeraInstanceError { source: tera::Error },

    #[error("Couldn't create template context")]
    CreateTeraContextError { source: tera::Error },

    #[error("Couldn't load components")]
    ComponentLoadError { source: FilesError },

    #[error("Couldn't register components with template engine")]
    ComponentRegisterError { source: tera::Error },

    #[error("Couldn't determine a render destination")]
    AmbiguousDestinationError { source: FilesError },

    #[error("Template engine error during render")]
    RenderError { source: tera::Error },
}

pub(crate) struct Renderer<'a> {
    pub template_engine: tera::Tera,
    pub base_context: tera::Context,
    pub build_config: &'a BuildConfig,
}

#[derive(Clone)]
pub(crate) enum RenderDestination {
    SectionIndex { directory: String },
    Explicit { directory: String, filename: String },
    Permalink { directory: String },
}

pub(crate) struct RenderPassDescriptor<T: Serialize> {
    pub render_name: String,
    pub base_template: &'static str,
    pub destination: RenderDestination,
    pub context: T,
}

pub(crate) struct Export {
    pub render_name: String,
    pub path: String,
}

impl<'a> Renderer<'a> {
    pub(crate) fn new(
        build_config: &'a BuildConfig,
        site_config: &'a SiteConfig,
    ) -> Result<Renderer<'a>, Error> {
        let log = build_config.debug;

        let templ_glob = &build_config.templates_glob;
        if log {
            println!("Loading templates from '{}'\n", templ_glob);
        }
        let mut template_engine = tera::Tera::new(templ_glob)
            .map_err(|e| Error::CreateTeraInstanceError { source: e })?;
        if log {
            println!("Loaded templates:");
            for name in template_engine.get_template_names() {
                println!("  - \"{}\"", name)
            }
            println!("");
        }

        // function for (map values) -> array
        template_engine.register_filter(
            "values",
            |value: &serde_json::Value,
             _: &std::collections::HashMap<String, serde_json::Value>|
             -> Result<serde_json::Value, tera::Error> {
                if let serde_json::Value::Object(obj) = value {
                    let mut values = Vec::new();
                    for (_, value) in obj {
                        values.push(value.clone());
                    }
                    Ok(serde_json::Value::Array(values))
                } else {
                    Err(tera::Error::call_filter("eh", "oh"))
                }
            },
        );

        let comp_glob = &build_config.components_glob;
        if log {
            println!("Loading components from '{}'\n", comp_glob);
        }
        let components = load_component_files(comp_glob, &build_config.source_dir_path)
            .map_err(|e| Error::ComponentLoadError { source: e })?;
        if log {
            println!("Loaded components:");
            for (name, _) in &components {
                println!("  - \"{}\"", name)
            }
            println!("");
        }
        template_engine
            .add_raw_templates(components)
            .map_err(|e| Error::ComponentRegisterError { source: e })?;

        let mut base_context = tera::Context::from_serialize(&site_config.context)
            .map_err(|e| Error::CreateTeraContextError { source: e })?;
        base_context.insert("posts", &site_config.posts);
        base_context.insert("pages", &site_config.pages);

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
        })
    }

    pub(crate) fn register_post_url(&mut self, name: &str, url: String) {
        self.register_url("posts", name, url);
    }
    pub(crate) fn register_page_url(&mut self, name: &str, url: String) {
        self.register_url("pages", name, url);
    }

    fn register_url(&mut self, container_name: &str, name: &str, url: String) {
        let mut context = self.base_context.clone().into_json();
        let container = context
            .get_mut(container_name)
            .expect("uhhhh")
            .as_object_mut()
            .expect("uuuh");
        let element = container
            .get_mut(name)
            .expect("uhhh")
            .as_object_mut()
            .expect("uhh");
        element.insert(String::from("url"), serde_json::Value::String(url));
        self.base_context = tera::Context::from_value(context).expect("uhh");
    }

    pub(crate) fn render_content<T: Serialize>(
        &mut self,
        desc: RenderPassDescriptor<T>,
    ) -> Result<Export, Error> {
        let destination = match &desc.destination {
            RenderDestination::SectionIndex { directory } => directory,
            RenderDestination::Permalink { directory } => directory,
            RenderDestination::Explicit { directory, .. } => directory,
        };
        let base_url = get_relative_path_string(&self.build_config.output_dir_path, destination)
            .map_err(|e| Error::AmbiguousDestinationError { source: e })?;

        // create context for render
        let mut context = self.base_context.clone();
        context.insert("base_url", &base_url);
        context.insert("render", &desc.context);

        // render
        print!("rendering '{}'...", &desc.render_name);
        let output = self
            .template_engine
            .render(&desc.base_template, &context)
            .map_err(|e| Error::RenderError { source: e })?;

        // 2nd render to resolve component includes
        let output = self
            .template_engine
            .render_str(&output, &context)
            .map_err(|e| Error::RenderError { source: e })?;

        print!("ok\n");

        // export
        let export = export(&desc.render_name, &output, desc.destination)?;

        Ok(export)
    }
}

fn export(
    name: &String,
    content: &String,
    destination: RenderDestination,
) -> Result<Export, Error> {
    let (filename, path) = match destination {
        RenderDestination::SectionIndex { directory } => (String::from("index.html"), directory),
        RenderDestination::Permalink { directory } => {
            let hash = Params::new().hash_length(12).hash(&content.as_bytes());
            let hash_string = Base64Url::encode_string(hash.as_bytes());
            let filename = format!("{}.html", hash_string);
            (filename, directory)
        }
        RenderDestination::Explicit {
            directory,
            filename,
        } => (format!("{}.html", filename), directory),
    };
    ensure_directory(&path).map_err(|e| Error::WriteExportError {
        source: e,
        name: name.clone(),
    })?;
    let path = format!("{}/{}", path, filename);
    println!("exporting {} -> {}", name, path);
    write_file_contents(&content, &path).map_err(|e| Error::WriteExportError {
        source: e,
        name: name.clone(),
    })?;

    Ok(Export {
        render_name: name.clone(),
        path,
    })
}
