use mdbook::book::{Book, BookItem};
use mdbook::errors::{Error, Result};
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use pulldown_cmark::{CodeBlockKind::*, Event, Options, Parser, Tag, TagEnd};
use pulldown_cmark_to_cmark::cmark;
use serde_json::json;
use skill_tree::SkillTree;
use std::fmt::Write;

#[derive(Default)]
pub struct SkillTreePreprocessor;

impl Preprocessor for SkillTreePreprocessor {
    fn name(&self) -> &str {
        "skill-tree"
    }

    fn run(&self, _ctx: &PreprocessorContext, mut book: Book) -> Result<Book> {
        let mut counter = 0;
        let mut res = None;
        book.for_each_mut(|item: &mut BookItem| {
            if let Some(Err(_)) = res {
                return;
            }

            if let BookItem::Chapter(chapter) = item {
                res = Some(add_skill_tree(&chapter.content, &mut counter).map(|md| {
                    chapter.content = md;
                }));
            }
        });

        res.unwrap_or(Ok(())).map(|_| book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

fn add_skill_tree(content: &str, counter: &mut usize) -> Result<String> {
    let mut buf = String::with_capacity(content.len());
    let mut skill_tree_content = String::new();
    let mut in_skill_tree_block = false;

    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);

    let events = Parser::new_ext(content, opts).map(|e| {
        // Detect the opening of a skill-tree fenced code block.
        if let Event::Start(Tag::CodeBlock(Fenced(code))) = e.clone() {
            if &*code == "skill-tree" {
                in_skill_tree_block = true;
                skill_tree_content.clear();
                return None;
            } else {
                return Some(e);
            }
        }

        // Pass through everything that isn't inside a skill-tree block.
        if !in_skill_tree_block {
            return Some(e);
        }

        // We are inside a skill-tree block — handle events.
        match e {
            // In pulldown-cmark 0.9+, Event::End carries a TagEnd which does not
            // include the code block's language string, so we just match the variant.
            Event::End(TagEnd::CodeBlock) => {
                in_skill_tree_block = false;

                let graphviz_text_or_err = SkillTree::parse(&skill_tree_content)
                    .and_then(|skill_tree| skill_tree.to_graphviz());

                let js_value = match graphviz_text_or_err {
                    Ok(text) => json!({
                        "dot_text": text,
                        "error": "",
                    }),

                    // FIXME -- we should serialize this into something that displays the error
                    // when rendered, rather than panicking the whole mdbook build.
                    Err(e) => panic!("encountered error {} parsing {:?}", e, skill_tree_content),
                };

                // Get a fresh id for this block.
                let id = *counter;
                *counter += 1;

                // Generate a "div" where the rendered SVG will be inserted.
                let mut html_code = String::new();
                write!(&mut html_code, "<div id='skill-tree-{}'>", id).unwrap();
                write!(&mut html_code, "</div>\n\n").unwrap();

                // Generate a script tag that queues this tree for rendering.
                write!(
                    &mut html_code,
                    r#"<script>
                    if (!window.SKILL_TREES) window.SKILL_TREES = [];
                    window.SKILL_TREES.push({{id:'skill-tree-{}', value:{}}});
                    </script>"#,
                    id, js_value
                )
                .unwrap();

                Some(Event::Html(html_code.into()))
            }
            Event::Text(code) => {
                skill_tree_content.push_str(&code);
                None
            }
            _ => Some(e),
        }
    });

    let events = events.filter_map(|e| e);

    // pulldown-cmark-to-cmark 11+ removed the options parameter.
    cmark(events, &mut buf)
        .map(|_| buf)
        .map_err(|err| Error::msg(format!("Markdown serialization failed: {}", err)))
}

#[cfg(test)]
mod test;
