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

fn render_error_html(err: &anyhow::Error, source: &str) -> String {
    let escaped_err = html_escape::encode_text(&err.to_string()).to_string();
    let escaped_src = html_escape::encode_text(source).to_string();

    format!(
        r#"
        <div class = "skill-tree-error" style = "border:2px solid #c0392b; border-radius:4px;padding:1em;margin:1em 0;background:#fdf0ef;color:#c0392b;font-family:monospace>
          <strong>skill-tree error</strong>
          <pre style="margin:0.5em 0 0; white-space:pre-wrap" > {escaped_err} </pre>
          <details style="margin-top:0.5em>
            <summary style="cursor:pointer;color:#888"> show source </summary>
            <pre style="margin:0.5em 0 0;color:#333;font-size:0.9em;white-space:pre-wrap> {escaped_src} </pre>
          </detils>
        </div>
      "#
    )
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
            Event::End(TagEnd::CodeBlock) => {
                in_skill_tree_block = false;
                let graphviz = SkillTree::parse(&skill_tree_content)
                    .and_then(|skill_tree| skill_tree.to_graphviz());

                let html_code = match graphviz {
                    Ok(dot_text) => {
                        let js_value = json!({
                          "dot_text": dot_text,
                          "error": "",
                        });

                        let id = *counter;
                        *counter += 1;

                        let mut html = String::new();

                        write!(&mut html, "<div id='skill-tree-{}'>", id).unwrap();
                        write!(&mut html, "</div>\n\n").unwrap();

                        write!(
                            &mut html,
                            r#"<script>
                          if (!window.SKILL_TREES) window.SKILL_TREES = [];
                          window.SKILL_TREES.push({{id: 'skill-tree-{}', value: {} }})
                          </script>"#,
                            id, js_value
                        )
                        .unwrap();

                        html
                    }
                    Err(err) => {
                        log::warn!("skill-tree: failed to render block: {}", err);

                        render_error_html(&err, &skill_tree_content)
                    }
                };

                Some(Event::Html(html_code.into()))
            }
            Event::Text(code) => {
                for code in code.chars() {
                    skill_tree_content.push(code);
                }
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
