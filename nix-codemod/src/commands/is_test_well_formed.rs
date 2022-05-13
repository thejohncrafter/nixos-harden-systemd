
use std::fs;
use std::error::Error;

use rnix::types::*;

use crate::walkers::*;

pub fn is_test_well_formed(test: &str) -> Result<(), Box<dyn Error>> {
    let content = fs::read_to_string(&test)?;
    let ast = rnix::parse(&content).as_result()?;
    
    let val = go_right_value(ast.root().inner().ok_or("parse error")?)?;

    if let Some(app_ext) = Apply::cast(val) {
        if let Some(app) = Apply::cast(app_ext.lambda().ok_or("parse error")?) {
            if let Ok(p) = expect_relative_path(app.value().ok_or("parse error")?) {
                if p == "./make-test-python.nix" || p == "../make-test-python.nix" {
                    println!("true");

                    return Ok(())
                }
            }
        }
    }

    println!("false");

    Ok(())
}

