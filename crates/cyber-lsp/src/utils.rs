use std::process::Command as cmd;
use serde::{Deserialize, Serialize};

mod doc_loader;
pub use doc_loader::*;
pub mod treehelper;

#[derive(Deserialize, Debug, Serialize, Clone)]
pub enum FileType {
    Dir,
    File,
}

impl std::fmt::Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileType::Dir => write!(f, "Dir"),
            FileType::File => write!(f, "File"),
        }
    }
}

// --| Notify Send ----------------
pub enum Type { Error, Warning, Info, }

impl ToString for Type {
    fn to_string(&self) -> String {
        match self {
            Type::Warning => "Warning".to_string(),
            Type::Error => "Error".to_string(),
            Type::Info => "Info".to_string(),
        }
    }
}

pub(crate) fn notify_send(input: &str, typeinput: Type) {
    cmd::new("notify-send").arg(typeinput.to_string()).arg(input).spawn().expect("Error");
}

