
use std::fs;
use std::error::Error;
use std::iter;

use clap::Parser;
use clap::Subcommand;

use serde_json;

use rnix::types::*;
use rnix::SyntaxNode;

use crate::walkers::*;
use crate::edit::*;

fn find_systemd_services(root: Root) -> Result<Vec<String>, Box<dyn Error>> {
    let x = root.inner().and_then(Lambda::cast).ok_or("root isn't a function")?;
    let x = go_right_value(x.body().ok_or("parse error")?)?;
    let x = decl_value(
        &[  "config".to_string(),
            "systemd".to_string(),
            "services".to_string(), ],
        x)?;
    
    match x {
        Some(DeclValue::Node(_)) => Err("Couldn't reduce")?,
        Some(DeclValue::PartialAttr { entries, .. }) => {
            let mut keys = entries.iter()
                .map(|(attr_name, _)| attr_name.first().unwrap().clone())
                .collect::<Vec<_>>();
            keys.sort();
            keys.dedup();
            Ok(keys)
        },
        None => Ok(vec!())
    }
}

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

pub fn list_systemd_services(module: &str, verbose: bool) -> Result<(), Box<dyn Error>> {
    let content = fs::read_to_string(&module)?;
    let ast = rnix::parse(&content).as_result()?;
    
    let declared_services: Vec<String> = find_systemd_services(ast.root())?
        .into_iter()
        .filter_map(|name| -> Option<String> {
            let decl = find_service_decl(ast.root(), &name).ok()?;
            let cfg = decl.clone().project("serviceConfig").ok()?;
            match cfg {
                Some(cfg) => if cfg.entries().ok()?.is_some() { Some(name) } else { None },
                None => {
                    let inherited: Vec<String> = AttrSet::cast(decl.value().clone()).ok_or("parse error").ok()?.inherits()
                        .map(|inherit| inherit.idents().map(|ident| ident.as_str().to_string()))
                        .flatten()
                        .collect();

                    if inherited.contains(&"serviceConfig".to_string()) {
                        None
                    } else {
                        Some(name)
                    }
                }
            }
        })
        .collect();

    if verbose {
        if declared_services.len() == 0 {
            println!("No systemd service");
            return Ok(())
        }

        println!("This file declares");
        for service in declared_services.iter() {
            println!(" * {}", service);
        }
    } else {
        println!("{}", serde_json::to_string(&declared_services)?);
    }

    Ok(())
}

