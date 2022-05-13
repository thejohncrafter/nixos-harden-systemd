
use std::error::Error;

//use rnix::types::*;
use rnix::SyntaxNode;
use rnix::SyntaxToken;
use rnix::SyntaxKind;
use rnix::NodeOrToken;

pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub replace: String,
}

pub fn apply_edits(mut edits: Vec<Edit>, text: &mut String) {
    // essentially https://github.com/rust-lang/rust-analyzer/blob/master/crates/text-edit/src/lib.rs
    edits.sort_by_key(|e| (e.start, e.end));
    if !Iterator::zip(edits.iter(), edits.iter().skip(1)).all(|(l, r)| l.start <= r.end) {
        panic!("Overlapping edits, this is a bug!")
    }

    for e in edits.into_iter().rev() {
        text.replace_range(e.start..e.end, &e.replace);
    }
}

/// Returns the end of the range to delete when deleting
/// the token `n`; and wether this range ends at the end
/// of a line.
pub fn find_span_end(n: &SyntaxNode) -> (usize, bool) {
    let mut end: usize = n.text_range().end().into();
    let mut node: NodeOrToken<SyntaxNode, SyntaxToken> = NodeOrToken::Node(n.clone());

    let is_line_end = loop {
        if let Some(n) = node.next_sibling_or_token() {
            node = n;
        }

        let s = match node.kind() {
            SyntaxKind::TOKEN_WHITESPACE => {
                match &node {
                    NodeOrToken::Token(token) => token.text(),
                    _ => unreachable!()
                }
            },
            _ => break false
        };

        match s.find('\n') {
            Some(i) => { end += i; break true },
            None => end = node.text_range().end().into()
        }
    };

    (end, is_line_end)
}

pub fn find_span_start(n: &SyntaxNode) -> usize {
    let mut start: usize = n.text_range().start().into();
    let mut node: NodeOrToken<SyntaxNode, SyntaxToken> = NodeOrToken::Node(n.clone());

    loop {
        if let Some(n) = node.prev_sibling_or_token() {
            node = n;
        }

        let s = match node.kind() {
            SyntaxKind::TOKEN_WHITESPACE => {
                match &node {
                    NodeOrToken::Token(token) => token.text(),
                    _ => unreachable!()
                }
            },
            _ => break
        };

        match s.rfind('\n') {
            Some(i) => {
                start = node.text_range().start().into();
                start += i;
                break
            },
            None => start = node.text_range().start().into()
        }
    }

    start
}

pub fn replace_node(n: &SyntaxNode, replace: String) -> Edit {
    let range = n.text_range(); 

    Edit {
        start: range.start().into(),
        end: range.end().into(),
        replace,
    }
}

pub fn remove_node(n: &SyntaxNode) -> Edit {
    let range = n.text_range();

    let (end, is_line_end) = find_span_end(n);
    let start = if is_line_end {
        find_span_start(n)
    } else { range.start().into() };

    Edit {
        start,
        end,
        replace: "".to_string(),
    }
}

/// Expects an `AttrSet` node
pub fn guess_indent(n: &SyntaxNode) -> Result<Option<usize>, Box<dyn Error>> {
    let n = {
        let mut n = n.first_child_or_token().ok_or("parse error")?;
        loop {
            if n.kind() == SyntaxKind::TOKEN_REC {
                n = n.next_sibling_or_token().ok_or("parse error")?
            } else if n.kind() == SyntaxKind::TOKEN_WHITESPACE {
                n = n.next_sibling_or_token().ok_or("parse error")?
            } else {
                break
            }
        }
        n
    };

    if n.kind() != SyntaxKind::TOKEN_CURLY_B_OPEN { Err("parse error")? }
    
    if let Some(n) = n.next_sibling_or_token() {
        if n.kind() == SyntaxKind::TOKEN_WHITESPACE {
            if let NodeOrToken::Token(token) = n {
                if let Some(i) = token.text().rfind("\n") {
                    //TODO: handle spaces & tabs ?
                    let end: usize = token.text_range().end().into();
                    let start: usize = token.text_range().start().into();
                    return Ok(Some(end - (start + i + 1)))
                }
            }
        }
    }

    Ok(None)
}

pub fn insert_at_set_end(n: &SyntaxNode, lines: &[String], indent: usize) -> Result<Edit, Box<dyn Error>> {
    let n = n.last_child_or_token().ok_or("parse error")?;
    if n.kind() != SyntaxKind::TOKEN_CURLY_B_CLOSE { Err("parse error")? }
    
    let mut spot: usize = n.text_range().start().into();

    if let Some(n) = n.prev_sibling_or_token() {
        if n.kind() == SyntaxKind::TOKEN_WHITESPACE {
            if let NodeOrToken::Token(token) = n {
                spot = token.text_range().start().into();
                if let Some(i) = token.text().rfind("\n") { spot += i }
            }
        }
    }

    let replace = lines.iter()
        .map(|s| format!("\n{:indent$}{}", "", s, indent=indent))
        .collect::<String>();

    Ok(Edit { start: spot, end: spot, replace })
}

pub fn insert_at_pattern_start(n: &SyntaxNode, text: String) -> Result<Edit, Box<dyn Error>> {
    let n = n.first_child_or_token().ok_or("parse error")?;
    if n.kind() != SyntaxKind::TOKEN_CURLY_B_OPEN { Err("parse error")? }

    Ok(Edit {
        start: n.text_range().end().into(),
        end: n.text_range().end().into(),
        replace: text,
    })
}

