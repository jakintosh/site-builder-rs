mod blocks;

use crate::files::{read_file_contents, Error as FilesError};
use blocks::Blocks;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("Couldn't load content")]
    ContentLoadError { source: FilesError },

    #[error("Couldn't load json")]
    JsonLoadError { source: FilesError },

    #[error("Couldn't parse json")]
    JsonParseError { source: serde_json::Error },

    #[error("Block header was malformed: '{reason}'")]
    MalformedBlockHeaderError { reason: String },

    #[error("Block content was malformed: '{reason}'")]
    MalformedBlockContentError { reason: String },
}

///
/// Site Context Structs

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct SiteContext {
    pub site_title: String,
    pub language_code: String,
    pub sections: Vec<SiteSection>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct SiteSection {
    pub name: String,
    pub site_path: String,
    pub index_content: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct SiteContentType {
    pub name: String,
    pub content_template: String,
}

///
/// Content Structs

#[derive(Serialize, Debug)]
pub(crate) struct JsonString {
    content: String,
}
impl TryFrom<JsonString> for serde_json::Value {
    type Error = Error;

    fn try_from(json: JsonString) -> Result<Self, Self::Error> {
        let json: serde_json::Value = serde_json::from_str(json.content.as_str())
            .map_err(|e| Error::JsonParseError { source: e })?;
        Ok(json)
    }
}

#[derive(Serialize, Debug)]
pub(crate) struct MarkdownString {
    content: String,
}
impl From<MarkdownString> for serde_json::Value {
    fn from(markdown: MarkdownString) -> Self {
        serde_json::Value::String(markdown.content)
    }
}

#[derive(Serialize, Debug)]
pub(crate) struct SamString {
    content: String,
}
impl From<SamString> for serde_json::Value {
    fn from(sam: SamString) -> Self {
        serde_json::Value::String(sam.content)
    }
}

#[derive(Serialize, Debug)]
pub(crate) struct HtmlString {
    content: String,
}
impl From<MarkdownString> for HtmlString {
    fn from(markdown: MarkdownString) -> Self {
        let mut html = String::new();
        let parser = pulldown_cmark::Parser::new(&markdown.content);
        pulldown_cmark::html::push_html(&mut html, parser);

        HtmlString { content: html }
    }
}
impl From<SamString> for HtmlString {
    fn from(sam: SamString) -> Self {
        let html = match sam.content.parse::<sam_rs::Element>() {
            Ok(element) => element.to_xml(0, false),
            Err(err) => panic!("{}", err),
        };
        HtmlString { content: html }
    }
}
impl From<HtmlString> for serde_json::Value {
    fn from(html: HtmlString) -> Self {
        serde_json::Value::String(html.content)
    }
}

pub(crate) enum Content {
    Post(Post),
    Page(Page),
}

#[derive(Deserialize)]
pub(crate) struct PostOption {
    metadata: MetadataOption,
    title: String,
    content: String,
}
#[derive(Serialize)]
pub(crate) struct Post {
    pub metadata: Metadata,
    pub title: String,
    pub html: String,
}
impl TryFrom<serde_json::Value> for Post {
    type Error = Error;

    fn try_from(json: serde_json::Value) -> Result<Self, Self::Error> {
        let post_opt = serde_json::from_value::<PostOption>(json)
            .map_err(|e| Error::JsonParseError { source: e })?;
        Ok(post_opt.into())
    }
}
impl From<PostOption> for Post {
    fn from(option: PostOption) -> Self {
        Post {
            metadata: option.metadata.into(),
            title: option.title,
            html: option.content,
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct PageOption {
    metadata: MetadataOption,
    title: String,
    content: String,
}
#[derive(Serialize)]
pub(crate) struct Page {
    pub metadata: Metadata,
    pub title: String,
    pub html: String,
}
impl TryFrom<serde_json::Value> for Page {
    type Error = Error;

    fn try_from(json: serde_json::Value) -> Result<Self, Self::Error> {
        let page_opt = serde_json::from_value::<PageOption>(json)
            .map_err(|e| Error::JsonParseError { source: e })?;
        Ok(page_opt.into())
    }
}
impl From<PageOption> for Page {
    fn from(option: PageOption) -> Self {
        Page {
            metadata: option.metadata.into(),
            title: option.title,
            html: option.content,
        }
    }
}

#[derive(Deserialize)]
struct MetadataOption {
    content_name: Option<String>,
    directory: Option<String>,
    author_name: String,
    published_date: String,
    updated_date: Option<String>,
    version: Option<u32>,
}
#[derive(Serialize)]
pub(crate) struct Metadata {
    pub content_name: String,
    pub directory: String,
    pub author_name: String,
    pub published_date: String,
    pub updated_date: String,
    pub version: u32,
}
impl From<MetadataOption> for Metadata {
    fn from(option: MetadataOption) -> Self {
        Metadata {
            content_name: option.content_name.unwrap_or(String::from("")),
            directory: option.directory.unwrap_or(String::from("")),
            author_name: option.author_name,
            updated_date: option.updated_date.unwrap_or(option.published_date.clone()),
            published_date: option.published_date,
            version: option.version.unwrap_or(1),
        }
    }
}

pub(crate) fn parse_blocks_file(path: impl AsRef<std::path::Path>) -> Result<Content, Error> {
    let file_contents =
        read_file_contents(&path).map_err(|e| Error::ContentLoadError { source: e })?;
    let (type_declaration, file_contents) = match file_contents.split_once("\n") {
        Some(strings) => strings,
        None => {
            return Err(Error::MalformedBlockHeaderError {
                reason: String::from("no newline in file"),
            })
        }
    };

    // println!("\nparsing blocks\n==============\n");
    let blocks: Blocks = file_contents.parse()?;
    // println!("\nblocks -> json\n==============\n");
    let json: serde_json::Value = blocks.try_into()?;

    // println!("\njson -> content\n===============\n");
    match type_declaration {
        "type::post" => Ok(Content::Post(json["post"].clone().try_into()?)),
        "type::page" => Ok(Content::Page(json["page"].clone().try_into()?)),
        _ => Err(Error::MalformedBlockHeaderError {
            reason: format!("invalid type header"),
        }),
    }
}

pub(crate) fn parse_json_string<T: DeserializeOwned>(json_str: &str) -> Result<T, Error> {
    serde_json::from_str(&json_str).map_err(|e| Error::JsonParseError { source: e })
}

pub(crate) fn parse_json_file<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, Error> {
    let file_contents = read_file_contents(path).map_err(|e| Error::JsonLoadError { source: e })?;
    parse_json_string(&file_contents)
}
