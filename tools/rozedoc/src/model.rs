// tools/rozedoc/src/model.rs
//
// Data model representing documented Roze packages, modules, functions,
// classes, enums, variants, traits, and type signatures.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocPackage {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub license: Option<String>,
    pub repository: Option<String>,
    pub readme: Option<String>,
    pub modules: Vec<DocModule>,
}

impl DocPackage {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            description: None,
            authors: Vec::new(),
            license: None,
            repository: None,
            readme: None,
            modules: Vec::new(),
        }
    }

    pub fn total_items(&self) -> usize {
        let mut count = 0;
        for m in &self.modules {
            count += m.functions.len() + m.classes.len() + m.enums.len() + m.traits.len() + m.constants.len();
            for c in &m.classes {
                count += c.fields.len() + c.methods.len();
            }
            for e in &m.enums {
                count += e.variants.len();
            }
        }
        count
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocModule {
    pub name: String,
    pub file_path: String,
    pub doc: Option<String>,
    pub functions: Vec<DocFunction>,
    pub classes: Vec<DocClass>,
    pub enums: Vec<DocEnum>,
    pub traits: Vec<DocTrait>,
    pub constants: Vec<DocConstant>,
    pub imports: Vec<String>,
}

impl DocModule {
    pub fn new(name: impl Into<String>, file_path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            file_path: file_path.into(),
            doc: None,
            functions: Vec::new(),
            classes: Vec::new(),
            enums: Vec::new(),
            traits: Vec::new(),
            constants: Vec::new(),
            imports: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocParam {
    pub name: String,
    pub type_name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocFunction {
    pub name: String,
    pub signature: String,
    pub type_params: Vec<String>,
    pub params: Vec<DocParam>,
    pub return_type: Option<String>,
    pub is_async: bool,
    pub is_extern: bool,
    pub doc: Option<String>,
    pub return_doc: Option<String>,
    pub throws_doc: Vec<String>,
    pub examples: Vec<String>,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocClass {
    pub name: String,
    pub type_params: Vec<String>,
    pub doc: Option<String>,
    pub fields: Vec<DocField>,
    pub methods: Vec<DocFunction>,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocField {
    pub name: String,
    pub type_name: Option<String>,
    pub doc: Option<String>,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocEnum {
    pub name: String,
    pub type_params: Vec<String>,
    pub doc: Option<String>,
    pub variants: Vec<DocVariant>,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocVariant {
    pub name: String,
    pub payload_types: Vec<String>,
    pub doc: Option<String>,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocTrait {
    pub name: String,
    pub doc: Option<String>,
    pub methods: Vec<DocFunction>,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocConstant {
    pub name: String,
    pub type_name: Option<String>,
    pub value: Option<String>,
    pub doc: Option<String>,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchIndexItem {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub module: String,
    pub signature: String,
    pub doc_summary: String,
    pub href: String,
}
