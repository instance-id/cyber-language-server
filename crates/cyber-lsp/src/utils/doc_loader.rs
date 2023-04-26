use once_cell::sync::Lazy;

pub mod loader {
    use tracing::info;
    use once_cell::sync::Lazy;
    use std::{collections::HashMap, path::PathBuf};
    use crate::datatypes::LanguageDoc;

    fn get_definitions() -> HashMap<String, LanguageDoc> {
        let path = std::env::current_exe().unwrap();
        let docs_path = path.parent().unwrap();
        info!("docs path: {:?}", docs_path);

        #[cfg(debug_assertions)]
        let path_glob = std::path::Path::new(docs_path).join("../../lang_docs/*/*.json"); 

        #[cfg(not(debug_assertions))]
        let path_glob = std::path::Path::new(docs_path).join("lang_docs/*/*.json"); 

        let mut doc_files: HashMap<String, LanguageDoc> = HashMap::new();

        info!("glob path: {:?}", &path_glob);

        let mut load_docs = || -> anyhow::Result<()> {
            for entry in glob::glob(&path_glob.to_str().unwrap())?.flatten() {
                let p = entry.as_path().to_str().unwrap();

                let name = p.split('/').collect::<Vec<&str>>().last().unwrap().to_string();
                let realname = name.split('.').collect::<Vec<&str>>().first().unwrap().to_string();
                
                doc_files.entry(realname.to_string()).or_insert_with(|| LanguageDoc {
                        docname: realname,
                        path: p.to_string(),
                    });
            }

            Ok(())
        };
        let _ = load_docs();
        doc_files
    }

    pub static LANGUAGE_DOCS: Lazy<Vec<LanguageDoc>> = Lazy::new(|| get_definitions().into_values().collect());
}

