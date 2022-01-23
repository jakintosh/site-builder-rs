use crate::files::read_file_contents;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;
use toml;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("Couldn't load toml")]
    TomlLoadError { source: std::io::Error },

    #[error("Couldn't parse toml")]
    TomlParseError { source: toml::de::Error },

    #[error("Couldn't load content")]
    ContentLoadError { source: std::io::Error },
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
pub(crate) struct ContentContext {
    pub content_type: Option<String>,
    pub content_title: Option<String>,
    pub data_type: Option<String>,
}

impl ContentContext {
    pub(crate) fn extend_context(&self, tera_context: &mut tera::Context) {
        if let Some(content_type) = &self.content_type {
            tera_context.insert("content_type", content_type);
        }
        if let Some(content_title) = &self.content_title {
            tera_context.insert("content_title", content_title);
        }
        if let Some(data_type) = &self.data_type {
            tera_context.insert("data_type", data_type);
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct SiteContext {
    pub site_title: String,
    pub language_code: String,
    pub content_type: String,
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

pub(crate) fn parse_toml_string<T: DeserializeOwned>(toml_str: &str) -> Result<T, Error> {
    toml::from_str(&toml_str).map_err(|e| Error::TomlParseError { source: e })
}

pub(crate) fn parse_toml_file<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, Error> {
    let file_contents = read_file_contents(path).map_err(|e| Error::TomlLoadError { source: e })?;
    parse_toml_string::<T>(&file_contents)
}

pub(crate) fn parse_html_file<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<(Option<T>, String), Error> {
    Ok(parse_content_file(path)?)
}

pub(crate) fn parse_markdown_file<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<(Option<T>, String), Error> {
    let (frontmatter, markdown) = parse_content_file(path)?;

    Ok((frontmatter, convert_markdown_to_html(&markdown)))
}

pub(crate) fn split_frontmatter_content<T: DeserializeOwned>(
    text: &String,
) -> Result<(Option<T>, String), Error> {
    enum State {
        WaitingForFrontmatter,
        IngestingFrontmatter,
        IngestingContent,
    }
    let mut state = State::WaitingForFrontmatter;
    let mut ingest = String::new();
    let mut frontmatter: Option<String> = None;
    let content: String;
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let mut should_ingest = false;
        state = match state {
            State::WaitingForFrontmatter => match line {
                "---" => State::IngestingFrontmatter,
                _ => {
                    should_ingest = true;

                    State::IngestingContent
                }
            },
            State::IngestingFrontmatter => match line {
                "---" => {
                    frontmatter = Some(ingest.clone());
                    ingest.clear();

                    State::IngestingContent
                }
                _ => {
                    should_ingest = true;

                    State::IngestingFrontmatter
                }
            },
            State::IngestingContent => {
                should_ingest = true;

                State::IngestingContent
            }
        };
        if should_ingest {
            ingest.push_str(line);
            ingest.push_str("\n");
        }
    }

    // assign remaining ingest to content
    content = ingest;

    let frontmatter_struct = match frontmatter {
        Some(frontmatter) => Some(parse_toml_string::<T>(&frontmatter)?),
        None => None,
    };

    Ok((frontmatter_struct, content))
}

pub(crate) fn wrap_content_in_template(content: &str, base_template: &str) -> String {
    let mut template = String::new();
    let template_header = format!("{{% extends \"{tmpl}\" %}}\n", tmpl = base_template);
    template.push_str(&template_header);
    template.push_str("{% block content -%}\n");
    template.push_str(&content);
    template.push_str("{%- endblock content %}\n");

    template
}

fn parse_content_file<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<(Option<T>, String), Error> {
    let file = read_file_contents(&path).map_err(|e| Error::ContentLoadError { source: e })?;
    let (frontmatter, content) = split_frontmatter_content::<T>(&file)?;

    Ok((frontmatter, content))
}

fn convert_markdown_to_html(markdown: &String) -> String {
    let mut html = String::new();
    let parser = pulldown_cmark::Parser::new(&markdown);
    pulldown_cmark::html::push_html(&mut html, parser);

    html
}

#[cfg(test)]
mod tests {
    use super::{parse_toml_string, ContentContext, SiteContentType, SiteContext, SiteSection};

    #[test]
    fn test_frontmatter_deserialize_toml() {
        let frontmatter_toml = "content_type = \"post\"\ncontent_title = \"title\"";
        let frontmatter: ContentContext =
            parse_toml_string(frontmatter_toml).expect("failed to parse toml");
        assert_eq!(
            frontmatter,
            ContentContext {
                content_title: Some("title".to_owned()),
                content_type: Some("post".to_owned()),
                data_type: None,
            }
        );
    }

    #[test]
    fn test_frontmatter_serialize_json() {
        let frontmatter = ContentContext {
            content_title: Some("title".to_owned()),
            content_type: Some("post".to_owned()),
            data_type: None,
        };
        let frontmatter_json =
            serde_json::to_value(frontmatter).expect("failed to serialize frontmatter");
        let content_title = frontmatter_json
            .get("content_title")
            .expect("couldn't get 'content_title'")
            .as_str()
            .expect("couldn't get string from 'content_title'");
        let content_type = frontmatter_json
            .get("content_type")
            .expect("couldn't get 'content_type'")
            .as_str()
            .expect("couldn't get string from 'content_type'");
        assert_eq!(content_title, "title");
        assert_eq!(content_type, "post");
    }

    #[test]
    fn test_site_config_deserialize_toml() {
        let toml = r#"site_title = "jakintosh"
language_code = "en-us"
content_type = "post"

[[sections]]
name = "home"
site_path = ""
index_content = "index.html"
priority = 1

[[sections]]
name = "posts"
site_path = "posts/"
index_content = "posts.html"
priority = 2

[[content_types]]
name = "post"
content_template = "post.tmpl""#;

        let site_config: SiteContext =
            parse_toml_string(&toml).expect("couldn't parse site config toml");
        assert_eq!(site_config.sections.len(), 2);
        assert_eq!(site_config.content_types.len(), 1);
        assert_eq!(site_config.site_title, "jakintosh");
    }

    #[test]
    fn test_site_config_serialize_json() {
        let site_config = SiteContext {
            site_title: "title".to_owned(),
            language_code: "en-US".to_owned(),
            content_type: "default.tmpl".to_owned(),
            sections: vec![SiteSection {
                name: "section".to_owned(),
                site_path: "section".to_owned(),
                index_content: "section.html".to_owned(),
                priority: 1,
            }],
            content_types: vec![SiteContentType {
                name: "post".to_owned(),
                content_template: "post.tmpl".to_owned(),
            }],
        };
        let site_config_json =
            serde_json::to_value(site_config).expect("failed to serialize site_config");
        let site_title = site_config_json
            .get("site_title")
            .expect("couldn't get 'site_title'")
            .as_str()
            .expect("couldn't get string from 'site_title'");

        let sections = site_config_json
            .get("sections")
            .expect("couldn't get 'sections'")
            .as_array()
            .expect("couldn't get array from 'sections'");
        assert_eq!(site_title, "title");
        assert_eq!(sections.len(), 1);
    }
}
