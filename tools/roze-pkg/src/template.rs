// src/template.rs
use std::fs;
use std::path::PathBuf;
use anyhow::Result;

pub struct Template {
    pub name: String,
    pub description: String,
    pub files: Vec<TemplateFile>,
}

#[derive(Debug, Clone)]
pub struct TemplateFile {
    pub path: String,
    pub content: String,
}

impl Template {
    pub fn get_default() -> Self {
        Self {
            name: "default".to_string(),
            description: "Default Roze project template".to_string(),
            files: vec![
                TemplateFile {
                    path: "src/main.roze".to_string(),
                    content: r#"// main.roze - Entry point for your Roze application
// Created by {{author}}
// Project: {{project_name}}

func main() {
    let project_name = "{{project_name}}";
    let author = "{{author}}";

    println("🌹 Hello from " + project_name + "!");
    println("Welcome to Roze programming language!");
    println("Created by " + author);

    let message = "This project was created with `roze new`";
    println(message);
}
"#.to_string(),
                },
                TemplateFile {
                    path: "roze.toml".to_string(),
                    content: r#"# Roze project configuration
name = "{{project_name}}"
version = "0.1.0"
description = "A Roze project"
authors = ["{{author}}"]
main = "src/main.roze"
"#.to_string(),
                },
                TemplateFile {
                    path: "README.md".to_string(),
                    content: vec![
                        "# {{project_name}}",
                        "",
                        "A Roze project created with `roze new`.",
                        "",
                        "## Build",
                        "```bash",
                        "roze-pkg build",
                        "```",
                        "",
                        "## Run",
                        "```bash",
                        "roze-pkg run",
                        "```",
                        "",
                        "## Test",
                        "```bash",
                        "roze-pkg test",
                        "```",
                    ].join("\n"),
                },
            ],
        }
    }

    pub fn get_web() -> Self {
        Self {
            name: "web".to_string(),
            description: "Web application template".to_string(),
            files: vec![
                TemplateFile {
                    path: "src/main.roze".to_string(),
                    content: r#"// Web server in Roze
import std::web;

func main() {
    let app = web.server();

    app.get("/", func(req, res) {
        res.html("<h1>🌹 Hello from Roze!</h1>");
    });

    app.get("/api/hello", func(req, res) {
        res.json({"message": "Hello from Roze API!"});
    });

    println("🌐 Server running on http://localhost:8080");
    app.listen(8080);
}
"#.to_string(),
                },
                TemplateFile {
                    path: "roze.toml".to_string(),
                    content: r#"name = "{{project_name}}"
version = "0.1.0"
description = "A Roze web application"
authors = ["{{author}}"]
main = "src/main.roze"

[dependencies]
std::web = "0.1"
"#.to_string(),
                },
                TemplateFile {
                    path: "README.md".to_string(),
                    content: vec![
                        "# {{project_name}}",
                        "",
                        "A web application built with Roze.",
                        "",
                        "## Run",
                        "```bash",
                        "roze-pkg run",
                        "```",
                        "",
                        "Visit http://localhost:8080",
                    ].join("\n"),
                },
            ],
        }
    }

    pub fn get_library() -> Self {
        Self {
            name: "library".to_string(),
            description: "Library template".to_string(),
            files: vec![
                TemplateFile {
                    path: "src/lib.roze".to_string(),
                    content: r#"// Library for {{project_name}}
// This is a Roze library

func hello(name: string) -> string {
    return "Hello " + name + " from {{project_name}}!";
}

func add(a: int, b: int) -> int {
    return a + b;
}
"#.to_string(),
                },
                TemplateFile {
                    path: "roze.toml".to_string(),
                    content: r#"name = "{{project_name}}"
version = "0.1.0"
description = "A Roze library"
authors = ["{{author}}"]
main = "src/lib.roze"
"#.to_string(),
                },
                TemplateFile {
                    path: "README.md".to_string(),
                    content: vec![
                        "# {{project_name}}",
                        "",
                        "A Roze library.",
                        "",
                        "## Usage",
                        "```roze",
                        "import {{project_name}};",
                        "",
                        "func main() {",
                        "    let msg = hello(\"World\");",
                        "    println(msg);",
                        "}",
                        "```",
                    ].join("\n"),
                },
            ],
        }
    }

    pub fn get_all_templates() -> Vec<Self> {
        vec![
            Self::get_default(),
            Self::get_web(),
            Self::get_library(),
        ]
    }

    pub fn apply(&self, project_name: &str, author: &str, output_dir: &PathBuf) -> Result<()> {
        fs::create_dir_all(output_dir.join("src"))?;

        for file in &self.files {
            let content = file.content
                .replace("{{project_name}}", project_name)
                .replace("{{author}}", author);

            let file_path = output_dir.join(&file.path);

            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }

            fs::write(&file_path, content)?;
        }

        Ok(())
    }
}