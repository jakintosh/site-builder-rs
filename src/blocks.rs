use crate::parsing::{Error, HtmlString, JsonString, MarkdownString, SamString};

pub(crate) struct Blocks(Vec<Block>);
impl TryFrom<Blocks> for serde_json::Value {
    type Error = Error;

    fn try_from(blocks: Blocks) -> Result<Self, Self::Error> {
        use serde_json::{json, Value};

        let blocks = blocks.0;
        let mut json = json!({});

        for block in blocks {
            // prep block path data
            let block_type_key = String::from(block.header.path.block_type.as_str());
            let mut child_path = block.header.path.path.clone();

            // create a path
            let mut path = Vec::new();
            path.push(block_type_key);
            path.append(&mut child_path);
            let last = path.pop();

            // unwrapped because we just created this above, known value
            let mut obj = json.as_object_mut().unwrap();

            // ensure path
            for component in &path {
                if None == obj.get_mut(component) {
                    obj.insert(component.clone(), json!({}));
                }
                obj = obj.get_mut(component).unwrap().as_object_mut().unwrap();
            }

            if let Some(last) = last {
                // create Value from content block
                let json: Value = block.content.try_into()?;
                // if it's a map, append to existing object or insert it if there isn't one
                if let Value::Object(mut map) = json {
                    if let Some(Value::Object(parent_map)) = obj.get_mut(&last) {
                        parent_map.append(&mut map);
                    } else {
                        obj.insert(last, Value::Object(map));
                    }
                // otherwise just insert
                } else {
                    obj.insert(last, json);
                }
            }
        }

        Ok(json)
    }
}
impl std::str::FromStr for Blocks {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        enum State {
            ParseHeader,
            WaitForContent { block_header: BlockHeader },
            BufferContent { block_header: BlockHeader },
        }
        let mut state = State::ParseHeader;
        let mut blocks: Vec<Block> = Vec::new();
        let mut buffer = String::new();
        let mut lines = s.lines();
        while let Some(line) = lines.next() {
            state = match state {
                State::ParseHeader => match line {
                    _ if line.is_empty() => state,
                    _ => {
                        let block_header = line
                            .parse()
                            .map_err(|e| Error::MalformedBlockHeaderError { reason: e })?;
                        State::WaitForContent { block_header }
                    }
                },
                State::WaitForContent { block_header } => match line {
                    _ if line.is_empty() => State::WaitForContent { block_header },
                    "+++" => State::BufferContent { block_header },
                    _ => {
                        return Err(Error::MalformedBlockContentError {
                            reason: format!(
                                "Expected content start marker ('+++') or blank line, found '{}'",
                                line
                            ),
                        })
                    }
                },
                State::BufferContent { block_header } => match line {
                    "+++" => {
                        let block = Block::new(block_header, buffer.clone());
                        blocks.push(block);
                        buffer.clear();
                        State::ParseHeader
                    }
                    _ => {
                        buffer.push_str(&format!("{}\n", line));
                        State::BufferContent { block_header }
                    }
                },
            };
        }

        // implicitly close an open content block at EOF
        if let State::BufferContent { block_header } = state {
            let block = Block::new(block_header, buffer);
            blocks.push(block);
        }

        Ok(Blocks(blocks))
    }
}

struct Block {
    header: BlockHeader,
    content: BlockContent,
}
impl Block {
    fn new(header: BlockHeader, content: String) -> Block {
        Block {
            content: BlockContent::transform(&header.encoding, content),
            header,
        }
    }
}

#[derive(Debug)]
struct BlockHeader {
    path: BlockPath,
    encoding: BlockEncoding,
}
impl std::str::FromStr for BlockHeader {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split(":").collect::<Vec<_>>()[..] {
            [block_path_str, encoding_str] => Ok(BlockHeader {
                path: block_path_str.parse()?,
                encoding: encoding_str.parse()?,
            }),
            _ => Err(format!(
                "Expected header format 'type:encoding', received {}",
                s
            )),
        }
    }
}

#[derive(Debug)]
struct BlockEncoding {
    source: Option<BlockEncodings>,
    encoding: BlockEncodings,
}
impl std::str::FromStr for BlockEncoding {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split("->").collect::<Vec<_>>()[..] {
            [source, encoding] => Ok(BlockEncoding {
                source: Some(BlockEncodings::from_str(source)?),
                encoding: BlockEncodings::from_str(encoding)?,
            }),
            [encoding] => Ok(BlockEncoding {
                source: None,
                encoding: BlockEncodings::from_str(encoding)?,
            }),
            _ => Err(format!("Invalid encoding transformation string")),
        }
    }
}

#[derive(Debug)]
enum BlockEncodings {
    Json,
    Markdown,
    Html,
    Sam,
}
impl std::str::FromStr for BlockEncodings {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(BlockEncodings::Json),
            "markdown" => Ok(BlockEncodings::Markdown),
            "html" => Ok(BlockEncodings::Html),
            "sam" => Ok(BlockEncodings::Sam),
            _ => Err(format!("'{}' is not a valid value for Formats", s)),
        }
    }
}

#[derive(Debug)]
enum BlockType {
    Metadata,
    Post,
    Page,
}
impl BlockType {
    fn as_str(&self) -> &'static str {
        match self {
            BlockType::Metadata => "metadata",
            BlockType::Post => "post",
            BlockType::Page => "page",
        }
    }
}
impl std::str::FromStr for BlockType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "metadata" => Ok(BlockType::Metadata),
            "post" => Ok(BlockType::Post),
            "page" => Ok(BlockType::Page),
            _ => Err(format!("'{}' is not a valid value for DataType", s)),
        }
    }
}

#[derive(Debug)]
struct BlockPath {
    block_type: BlockType,
    path: Vec<String>,
}
impl std::str::FromStr for BlockPath {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut components = s.split(".");

        let block_type = match components.next() {
            Some(base) => base.parse()?,
            None => {
                return Err(format!(
                    "Expected at least one path token to declare block type"
                ))
            }
        };

        let mut path: Vec<String> = vec![];
        while let Some(component) = components.next() {
            if component.is_empty() {
                return Err(format!("Found empty component in block path '{}'", s));
            }
            path.push(String::from(component));
        }

        Ok(BlockPath { block_type, path })
    }
}

#[derive(Debug)]
enum BlockContent {
    Json(JsonString),
    Markdown(MarkdownString),
    Html(HtmlString),
    Sam(SamString),
}
impl BlockContent {
    fn transform(encoding: &BlockEncoding, content: String) -> BlockContent {
        match encoding.encoding {
            BlockEncodings::Json => BlockContent::Json(JsonString { content }),
            BlockEncodings::Markdown => BlockContent::Markdown(MarkdownString { content }),
            BlockEncodings::Html => match encoding.source {
                Some(BlockEncodings::Markdown) => {
                    BlockContent::Html((MarkdownString { content }).into())
                }
                Some(BlockEncodings::Sam) => BlockContent::Html((SamString { content }).into()),
                _ => BlockContent::Html(HtmlString { content }),
            },
            BlockEncodings::Sam => BlockContent::Sam(SamString { content }),
        }
    }
}
impl TryFrom<BlockContent> for serde_json::Value {
    type Error = Error;

    fn try_from(value: BlockContent) -> Result<Self, Self::Error> {
        match value {
            BlockContent::Json(json) => json.try_into(),
            BlockContent::Markdown(md) => Ok(md.into()),
            BlockContent::Html(html) => Ok(html.into()),
            BlockContent::Sam(sam) => Ok(sam.into()),
        }
    }
}
