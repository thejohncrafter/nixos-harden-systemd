
use std::fs;
use std::error::Error;
use std::iter;

use serde_json;

use rnix::types::*;
use rnix::SyntaxNode;

use crate::walkers::*;
use crate::edit::*;

fn find_service_decl(root: Root, service: &str) -> Result<DeclValue, Box<dyn Error>> {
    let x = root.inner().and_then(Lambda::cast).ok_or("root isn't a function")?;
    let x = go_right_value(x.body().ok_or("parse error")?)?;
    decl_value(
        &[  "config".to_string(),
            "systemd".to_string(),
            "services".to_string(),
            service.to_string()],
        x)?.ok_or(format!("config.systemd.services.{} is not declared", service).into())
}

fn modify_attribute_set(n: SyntaxNode, replacements: &[(String, String)]) -> Result<Vec<Edit>, Box<dyn Error>> {
    let n = match ParsedType::try_from(n)? {
        ParsedType::AttrSet(o) => o,
        _ => panic!("expected an attribute set")
    };

    let to_remove = n.entries().map(|e| {
        let keys = e.key().ok_or("parse error")?.path()
            .map(parse_ident)
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;

        for (k, _) in replacements.iter() {
            if keys == [Some(k.to_string())] {
                return Ok(Some(e))
            }
        }

        Ok(None)
    }).collect::<Result<Vec<_>, Box<dyn Error>>>()?.into_iter().filter_map(|o| o);


    let mut edits = Vec::new();

    for n in to_remove {
        edits.push(remove_node(&n.node()));
    }

    let indent = guess_indent(n.node())?.unwrap_or(0);
    let lines = replacements.iter()
        .map(|(k, v)| format!("{} = {};", k, v))
        .collect::<Vec<String>>();
    edits.push(insert_at_set_end(n.node(), &lines, indent)?);

    Ok(edits)
}

fn merge_decls(
    n: &SyntaxNode,
    prefix: &[String],
    entries: &[(Vec<String>, DeclKV)],
    replacements: &[(String, String)]
) -> Result<Vec<Edit>, Box<dyn Error>> {
    let indent = guess_indent(n)?.unwrap_or(0);
    let lines = iter::once(format!("{} = {{", prefix.join(".")))
        .chain(entries.iter().filter_map(|(key, DeclKV { value, .. })| {
            for (k, _) in replacements {
                if key == &[k.to_string()] {
                    return None
                }
            }
            Some(format!("  {} = {};", key.join("."), value))
        }))
        .chain(replacements.iter()
            .map(|(k, v)| format!("  {} = {};", k, v)))
        .chain(iter::once("};".to_string()))
        .collect::<Vec<String>>();
    let edits = entries.iter()
        .map(|(_, DeclKV { node, .. })| remove_node(&node))
        .chain(iter::once(insert_at_set_end(n, &lines, indent)?))
        .collect::<Vec<Edit>>();

    Ok(edits)
}

fn add_attribute_decl(n: &SyntaxNode, prefix: &[String], replacements: &[(String, String)]) -> Result<Vec<Edit>, Box<dyn Error>> {
    let inherited: Vec<String> = AttrSet::cast(n.clone()).ok_or("parse error")?.inherits()
        .map(|inherit| inherit.idents().map(|ident| ident.as_str().to_string()))
        .flatten()
        .collect();

    if inherited.contains(&"serviceConfig".to_string()) {
        Err("can't replace an attribute that is inherited")?
    }

    let indent = guess_indent(n)?.unwrap_or(0);
    let lines = iter::once(format!("{}serviceConfig = {{",
            prefix.iter().map(|k| format!("{}.", k)).collect::<String>()))
        .chain(replacements.iter()
            .map(|(k, v)| format!("  {} = {};", k, v))
            .collect::<Vec<String>>())
        .chain(iter::once("};".to_string()))
        .collect::<Vec<String>>();
    let edit = insert_at_set_end(n, &lines, indent)?;

    Ok(vec![edit])
}

pub fn edit_systemd_service(module: &str, service: &str, options: &str, verbose: bool) -> Result<(), Box<dyn Error>> {
    let content = fs::read_to_string(&module)?;
    let ast = rnix::parse(&content).as_result()?;

    let decl = find_service_decl(ast.root(), &service)?;
    let cfg = decl.clone().project("serviceConfig")?;

    let options: Vec<(String, String)> = {
        let content = fs::read_to_string(&options)?;
        serde_json::from_str(&content)?
    };
    
    let edits = match cfg {
        Some(DeclValue::Node(n)) => {
            if verbose {
                println!("modify entries in already declared {}.serviceConfig", service);
            }

            modify_attribute_set(n.value, &options)?
        },
        Some(DeclValue::PartialAttr { node, prefix, entries }) => {
            if verbose {
                println!("merge declarations in {} = {{ ... }}", prefix.join("."));
            }

            merge_decls(&node, &prefix, &entries, &options)?
        },
        None => {
            if verbose {
                println!("add {}.serviceConfig",
                    decl.prefix().iter().map(|k| format!("{}.", k)).collect::<String>());
            }

            add_attribute_decl(&decl.value(), decl.prefix(), &options)?
        },
    };

    let mut text = content.clone();
    
    if verbose {
        println!("{}", text);
    }

    apply_edits(edits, &mut text);

    print!("{}", text);

    Ok(())
}

fn add_passthru_arg(root: Root) -> Result<Vec<Edit>, Box<dyn Error>> {
    let n = root.inner().and_then(Lambda::cast).ok_or("root isn't a function")?;
    let n = n.arg().and_then(Pattern::cast).ok_or("root function's argument isn't a pattern")?;

    let already_defined = n.entries()
        .filter_map(|x| x.name())
        .any(|ident| ident.as_str() == "systemdPassthru");

    if !already_defined {
        Ok(vec!(insert_at_pattern_start(n.node(), " systemdPassthru,".to_string())?))
    } else {
        Ok(vec!())
    }
}

fn systemd_hooks_edits(root: Root, service: &str, option_names: &[String]) -> Result<Vec<Edit>, Box<dyn Error>> {
    let mut edits = vec!();

    edits.append(&mut add_passthru_arg(root.clone())?);

    let decl = find_service_decl(root, &service)?;
    let cfg = decl.clone().project("serviceConfig")?;

    fn maybe_quote(name: &str) -> String {
        if name.chars().all(|c| c.is_alphanumeric()) {
            name.to_string()
        } else {
            format!("\"{}\"", name)
        }
    }

    let options: Vec<(String, String)> = option_names.iter()
        .map(|name| (name.clone(), format!("systemdPassthru.{}.{}", maybe_quote(service), name)))
        .collect();

    edits.append(&mut match cfg {
        Some(DeclValue::Node(n)) =>
            modify_attribute_set(n.value, &options)?,
        Some(DeclValue::PartialAttr { node, prefix, entries }) =>
            merge_decls(&node, &prefix, &entries, &options)?,
        None =>
            add_attribute_decl(&decl.value(), decl.prefix(), &options)?,
    });

    Ok(edits)
}

pub fn insert_systemd_hooks(module: &str, service: &str, option_names: &str) -> Result<(), Box<dyn Error>> {
    let content = fs::read_to_string(&module)?;
    let ast = rnix::parse(&content).as_result()?;

    let option_names: Vec<String> = {
        let content = fs::read_to_string(&option_names)?;
        serde_json::from_str(&content)?
    };

    let edits = systemd_hooks_edits(ast.root(), service, &option_names)?;

    let mut text = content.clone();
    apply_edits(edits, &mut text);

    print!("{}", text);

    Ok(())
}

#[cfg(test)]
mod edit_tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn test_case(input: &str, output: &str) {
        let options: &[(String, String)] = &[
            ("a".to_string(), "false".to_string()),
            ("c".to_string(), "true".to_string()),
        ];

        let ast = rnix::parse(input).as_result().unwrap();
        let decl = find_service_decl(ast.root(), "codemod").unwrap();
        let cfg = decl.clone().project("serviceConfig").unwrap();

        let edits = match cfg {
            Some(DeclValue::Node(n)) =>
                modify_attribute_set(n.value, &options).unwrap(),
            Some(DeclValue::PartialAttr { node, prefix, entries }) =>
                merge_decls(&node, &prefix, &entries, &options).unwrap(),
            None =>
                add_attribute_decl(&decl.value(), decl.prefix(), &options).unwrap(),
        };

        let mut text = input.to_string();
        apply_edits(edits, &mut text);

        assert_eq!(text, output);
    }

    #[test]
    fn test_add_entry() {
        test_case("
        {}: {
          config.systemd.services.codemod = {
            u = true;
          };
        }
        ", "
        {}: {
          config.systemd.services.codemod = {
            u = true;
            serviceConfig = {
              a = false;
              c = true;
            };
          };
        }
        ");
    }

    #[test]
    fn test_deep_merge_entries() {
        test_case("
        {}: {
          config.systemd.services.codemod.u = true;
          config.systemd.services.codemod.serviceConfig.a = true;
          config.systemd.services.codemod.serviceConfig.b = true;
        }
        ", "
        {}: {
          config.systemd.services.codemod.u = true;
          config.systemd.services.codemod.serviceConfig = {
            b = true;
            a = false;
            c = true;
          };
        }
        ");
    }

    #[test]
    fn test_merge_add() {
        test_case("
        {}: {
          config.systemd.services.codemod.a = true;
          config.systemd.services.codemod.b = true;
        }
        ", "
        {}: {
          config.systemd.services.codemod.a = true;
          config.systemd.services.codemod.b = true;
          config.systemd.services.codemod.serviceConfig = {
            a = false;
            c = true;
          };
        }
        ");
    }

    #[test]
    fn test_merge_entries() {
        test_case("
        {}: {
          config.systemd.services.codemod = {
            serviceConfig.a = true;
            serviceConfig.b = true;
          };
        }
        ", "
        {}: {
          config.systemd.services.codemod = {
            serviceConfig = {
              b = true;
              a = false;
              c = true;
            };
          };
        }
        ");
    }

    #[test]
    fn test_modify_entries() {
        test_case("
        {}:
        {
            config.systemd.services.codemod.serviceConfig = {
                a = true;
                b = true;
            };
        }
        ", "
        {}:
        {
            config.systemd.services.codemod.serviceConfig = {
                b = true;
                a = false;
                c = true;
            };
        }
        ");
    }
}

#[cfg(test)]
mod hooks_test {
    use pretty_assertions::assert_eq;

    use super::*;

    fn base_test_case(service: &str, input: &str, output: &str) {
        let option_names = &["a".to_string(), "c".to_string()];
        let ast = rnix::parse(input).as_result().unwrap();

        let edits = systemd_hooks_edits(ast.root(), service, option_names).unwrap();
        let mut text = input.to_string();
        apply_edits(edits, &mut text);

        assert_eq!(text, output);
    }
    
    fn test_case(input: &str, output: &str) {
        base_test_case("codemod", input, output)
    }

    #[test]
    fn test_systemd_hooks() {
        test_case("
        { pkgs, ... }: {
          config.systemd.services.codemod.serviceConfig = {
            a = true;
            b = true;
          };
        }
        ", "
        { systemdPassthru, pkgs, ... }: {
          config.systemd.services.codemod.serviceConfig = {
            b = true;
            a = systemdPassthru.codemod.a;
            c = systemdPassthru.codemod.c;
          };
        }
        ");
    }

    #[test]
    fn test_hook_idempotent() {
        test_case("
        { systemdPassthru, pkgs, ... }: {
          config.systemd.services.codemod.serviceConfig = {
            b = true;
            a = systemdPassthru.codemod.a;
            c = systemdPassthru.codemod.c;
          };
        }
        ", "
        { systemdPassthru, pkgs, ... }: {
          config.systemd.services.codemod.serviceConfig = {
            b = true;
            a = systemdPassthru.codemod.a;
            c = systemdPassthru.codemod.c;
          };
        }
        ");
    }

    #[test]
    fn test_non_alphanumeric() {
        base_test_case("codemod@", "
        { systemdPassthru, pkgs, ... }: {
          config.systemd.services.\"codemod@\".serviceConfig = {
            b = true;
            a = systemdPassthru.codemod.a;
            c = systemdPassthru.codemod.c;
          };
        }
        ", "
        { systemdPassthru, pkgs, ... }: {
          config.systemd.services.\"codemod@\".serviceConfig = {
            b = true;
            a = systemdPassthru.\"codemod@\".a;
            c = systemdPassthru.\"codemod@\".c;
          };
        }
        ");
    }
}

