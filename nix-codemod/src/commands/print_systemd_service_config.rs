
use std::fs;
use std::error::Error;

use serde_json;

use rnix::types::*;
use rnix::SyntaxNode;

use crate::walkers::*;

static bool_options: &'static [&'static str] = &[
    "PrivateDevices",
    "PrivateMounts",
    "PrivateNetwork",
    "PrivateTmp",
    "PrivateUsers",
    "ProtectControlGroups",
    "ProtectKernelModules",
    "ProtectKernelTunables",
    "ProtectKernelLogs",
    "ProtectClock",
    "ProtectHostname",
    "LockPersonality",
    "MemoryDenyWriteExecute",
    "NoNewPrivileges",
    //"Delegate", -- inverted, so not here!
    "RestrictRealtime",
    "RestrictSUIDSGID",
];

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

pub fn print_systemd_service_config(
    module: &str,
    service: &str,
    verbose: bool
) -> Result<(), Box<dyn Error>> {
    let content = fs::read_to_string(&module)?;
    let ast = rnix::parse(&content).as_result()?;

    let decl = find_service_decl(ast.root(), &service)?;
    let cfg = decl.project("serviceConfig")?;

    let entries = if let Some(entries) = cfg.map(DeclValue::entries).transpose()?.unwrap_or(Some(vec!())) {
        entries
    } else {
        Err("serviceConfig is not an attribute set")?
    };

    let configured: Vec<String> = entries.iter()
        .filter_map(|(key, _)| if key.len() == 1 { Some(key.first().unwrap().clone()) } else { None })
        .collect();

    if verbose {
        for (key, DeclKV { value, .. }) in entries.into_iter() {
            print!(" * {} = ", key.join("."));

            match parse_cfg_value(value)? {
                CfgValue::Str(s) => {
                    println!("{}", s);
                },
                CfgValue::Bool(b) => {
                    println!("{}", b);
                },
                CfgValue::List(elems) => {
                    println!("[ {} ]", elems.join(" "));
                },
                CfgValue::NotReduced => {
                    println!("<not reduced>");
                },
            }
        }

        println!();
    }

    let blank_options: Vec<&str> = bool_options.iter()
        .filter(|opt| !configured.contains(&opt.to_string()))
        .map(|s| *s)
        .collect();

    println!("{}", serde_json::to_string(&blank_options)?);

    Ok(())
}

