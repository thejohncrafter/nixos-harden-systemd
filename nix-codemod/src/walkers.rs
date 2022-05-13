
use std::error::Error;

use rnix::types::*;
use rnix::value;
use rnix::SyntaxNode;
use rnix::StrPart;

pub fn expect_relative_path(n: SyntaxNode) -> Result<String, Box<dyn Error>> {
    let v = Value::cast(n)
        .ok_or("unexpected file structure")?
        .to_value()?;
    if let value::Value::Path(value::Anchor::Relative, s) = v {
        Ok(s)
    } else {
        Err("expected a relative path")?
    }
}

pub fn parse_ident(n: SyntaxNode) -> Result<Option<String>, Box<dyn Error>> {
    match ParsedType::try_from(n)? {
        ParsedType::Ident(n) => {
            Ok(Some(n.as_str().to_string()))
        },
        _ => Ok(None)
    }
}

fn extract_simple_string(n: Str) -> Option<String> {
    if n.parts().len() == 1 {
        if let Some(StrPart::Literal(s)) = n.parts().first() {
            Some(s.clone())
        } else {
            panic!("Isn't that case unreachable?")
        }
    } else {
        None
    }
}

pub enum CfgValue {
    Str(String),
    Bool(bool),
    List(Vec<String>),
    NotReduced,
}

pub fn parse_cfg_value(n: SyntaxNode) -> Result<CfgValue, Box<dyn Error>> {
    match ParsedType::try_from(n)? {
        ParsedType::Str(n) => {
            if let Some(s) = extract_simple_string(n) {
                Ok(CfgValue::Str(s.clone()))
            } else {
                Ok(CfgValue::NotReduced)
            }
        },
        ParsedType::Ident(n) => {
            match n.as_str() {
                "true" => Ok(CfgValue::Bool(true)),
                "false" => Ok(CfgValue::Bool(false)),
                _ => Ok(CfgValue::NotReduced)
            }
        },
        ParsedType::List(n) => {
            let elems = n.items()
                .map(|n| extract_simple_string(Str::cast(n)?))
                .collect::<Option<Vec<String>>>();
            match elems {
                Some(elems) => Ok(CfgValue::List(elems)),
                None => Ok(CfgValue::NotReduced)
            }
        },
        _ => Ok(CfgValue::NotReduced)
    }
}

pub fn parse_ident_select(n: SyntaxNode) -> Result<Vec<String>, Box<dyn Error>> {
    struct Visitor {
        idents: Vec<String>
    }

    impl Visitor {
        fn visit(&mut self, n: SyntaxNode) -> Result<(), Box<dyn Error>> {
            match ParsedType::try_from(n)? {
                ParsedType::Ident(n) => {
                    self.idents.push(n.as_str().to_string());
                    Ok(())
                },
                ParsedType::Select(n) => {
                    self.idents.push(
                        parse_ident(n.index().ok_or("parse error")?)?
                        .ok_or("expected an identifier")?);
                    self.visit(n.set().ok_or("parse error")?)
                },
                _ => Err("parse error")?
            }
        }
    }

    let mut v = Visitor { idents: vec!() };
    v.visit(n)?;
    v.idents.reverse();

    Ok(v.idents)
}

pub fn go_right_value(n: SyntaxNode) -> Result<SyntaxNode, Box<dyn Error>> {
    match ParsedType::try_from(n) {
        Ok(ParsedType::LetIn(n)) => {
            go_right_value(n.body().ok_or("parse error")?)
        },
        Ok(ParsedType::With(n)) => {
            go_right_value(n.body().ok_or("parse error")?)
        },
        Ok(ParsedType::Apply(n)) => {
            if let Ok(p) = parse_ident_select(n.lambda().ok_or("parse error")?) {
                if p == ["mkMerge"] {
                    todo!("handle mkMerge")
                } else {
                    Ok(n.node().clone())
                }
            } else {
                match ParsedType::try_from(n.lambda().ok_or("parse error")?)? {
                    ParsedType::Apply(n1) => {
                        if let Ok(p) = parse_ident_select(n1.lambda().ok_or("parse error")?) {
                            if p == ["mkIf"] {
                                n.value().ok_or("parse error".into())
                            } else {
                                Ok(n.node().clone())
                            }
                        } else {
                            Ok(n.node().clone())
                        }
                    },
                    _ => Ok(n.node().clone())
                }
            }
        },
        Ok(n) => Ok(n.node().clone()),
        _ => Err("parse error")?
    }
}

#[derive(Clone)]
pub struct DeclKV {
    /// The KeyValue node that declared this entry
    pub node: SyntaxNode,
    /// The name of the node in the attribute set it originates from 
    pub key: Vec<String>,
    pub value: SyntaxNode,
}

pub fn attrset_entries(n: SyntaxNode) -> Result<Option<Vec<DeclKV>>, Box<dyn Error>> {
    match ParsedType::try_from(n)? {
        ParsedType::AttrSet(n) => n.entries()
            .map(|entry| {
                let keys = entry.key().ok_or("parse error")?.path().map(|n| -> Result<String, Box<dyn Error>> {
                    match ParsedType::try_from(n)? {
                        ParsedType::Ident(n) => {
                            Ok(n.as_str().to_string())
                        },
                        ParsedType::Str(n) => {
                            if n.parts().len() == 1 {
                                if let Some(StrPart::Literal(s)) = n.parts().first() {
                                    Ok(s.clone())
                                } else {
                                    panic!("Isn't that case unreachable?")
                                }
                            } else {
                                todo!()
                            }
                        },
                        _ => Err("Unexpected node type when unrolling keys")?
                    }
                }).collect::<Result<Vec<String>, _>>()?;
                Ok(DeclKV {
                    node: entry.node().clone(),
                    key: keys,
                    value: entry.value().ok_or("parse error")?,
                })
            })
            .collect::<Result<Vec<DeclKV>, _>>().map(Some),
        _ => Ok(None)
    }
}

#[derive(Clone)]
pub enum DeclValue {
    Node(DeclKV),
    PartialAttr {
        node: SyntaxNode,
        prefix: Vec<String>,
        entries: Vec<(Vec<String>, DeclKV)>
    },
}

impl DeclValue {
    pub fn value(&self) -> &SyntaxNode {
        match self {
            DeclValue::Node(kv) => &kv.value,
            DeclValue::PartialAttr { node, .. } => &node,
        }
    }

    pub fn prefix(&self) -> &[String] {
        match self {
            DeclValue::Node(_) => &[],
            DeclValue::PartialAttr { prefix, .. } => &prefix,
        }
    }

    pub fn entries(self) -> Result<Option<Vec<(Vec<String>, DeclKV)>>, Box<dyn Error>> {
        match self {
            DeclValue::Node(n) => Ok(attrset_entries(n.value)?
                .map(|entries| entries.into_iter().map(|entry| (entry.key.clone(), entry)).collect())),
            DeclValue::PartialAttr { entries, .. } => Ok(Some(entries)),
        }
    }

    pub fn project(self, p: &str) -> Result<Option<DeclValue>, Box<dyn Error>> {
        match self {
            DeclValue::Node(n) => {
                let n = go_right_value(n.value)?;
                decl_value(&[p.to_string()], n)
            },
            DeclValue::PartialAttr { node, mut prefix, entries } => {
                let mut v: Vec<_> = entries.into_iter()
                    .filter(|(attr_name, _)| if let Some(q) = attr_name.first() { p == q } else { false })
                    .map(|(mut attr_name, DeclKV { node, key, value })| {
                        attr_name.remove(0);
                        (attr_name, DeclKV { node, key, value })
                    }).collect();
                if v.is_empty() {
                    Ok(None)
                } else if v.len() == 1 && v.first().unwrap().0.len() == 0 {
                    let (_, kv) = v.remove(0);
                    Ok(Some(DeclValue::Node(kv)))
                } else {
                    prefix.push(p.to_string());
                    Ok(Some(DeclValue::PartialAttr { node, prefix, entries: v }))
                }
            }
        }
    }
}

pub fn decl_value(path: &[String], n: SyntaxNode) -> Result<Option<DeclValue>, Box<dyn Error>> {
    struct ValHolder {
        val: Option<DeclValue>
    }

    impl ValHolder {
        fn merge(&mut self, v: DeclValue) -> Result<(), Box<dyn Error>> {
            match (&mut self.val, v) {
                (None, v) => self.val = Some(v),
                (Some(DeclValue::PartialAttr { entries, .. }),
                    DeclValue::PartialAttr { entries: mut entries2, .. }) => {
                    entries.append(&mut entries2);
                },
                _ => Err("defined multiple times")?,
            }
            Ok(())
        }
    }

    let mut holder = ValHolder { val: None };

    for kv in attrset_entries(n.clone())?.ok_or("couldn't reduce")? {
        // Check the paths match
        if !Iterator::zip(path.iter(), kv.key.iter()).all(|(p, q)| p == q) {
            continue
        }

        match Ord::cmp(&path.len(), &kv.key.len()) {
            std::cmp::Ordering::Less => {
                holder.merge(DeclValue::PartialAttr {
                    node: n.clone(),
                    prefix: path.into(),
                    entries: vec![(
                        kv.key[path.len()..].into(),
                        kv
                    )],
                })?
            },
            std::cmp::Ordering::Equal => {
                holder.merge(DeclValue::Node(kv))?
            },
            std::cmp::Ordering::Greater => {
                let x = go_right_value(kv.value)?;
                let x = decl_value(&path[kv.key.len()..], x)?;
                if let Some(x) = x { holder.merge(x)? }
            },
        }
    }

    Ok(holder.val)
}

