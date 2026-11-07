// src/template.rs
use std::fs;
use std::path::PathBuf;
use anyhow::Result;

pub struct Template {
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
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

    let message = "This project was created with `bsr new`";
    println(message);
}
"#.to_string(),
                },
                TemplateFile {
                    path: "bsr.mfz".to_string(),
                    content: r#"# bsr.mfz - Basir Project Manifest (Mohammad Firoze)
name = "{{project_name}}"
version = "0.1.0"
edition = "2026"
description = "A Roze project"
authors = ["{{author}}"]
main = "src/main.roze"
"#.to_string(),
                },
                TemplateFile {
                    path: "tests/integration_test.roze".to_string(),
                    content: r#"// tests/integration_test.roze - Integration tests for {{project_name}}

func assert_eq_int(actual: int, expected: int, msg: string) {
    if actual != expected {
        println("❌ Assertion failed: " + msg);
    } else {
        println("✅ " + msg);
    }
}

func test_basic_arithmetic() {
    let result = 1 + 1;
    assert_eq_int(result, 2, "1 + 1 == 2");
}

func main() {
    println("🧪 Running integration tests for {{project_name}}...");
    test_basic_arithmetic();
    println("All integration tests passed!");
}
"#.to_string(),
                },
                TemplateFile {
                    path: "benches/bench_main.roze".to_string(),
                    content: r#"// benches/bench_main.roze - Benchmark harness for {{project_name}}

func run_benchmark() {
    println("⏱️  Running benchmarks for {{project_name}}...");
    let sum = 0;
    let i = 0;
    while i < 100000 {
        sum = sum + i;
        i = i + 1;
    }
    if sum > 0 {
        println("✅ Benchmark iterations complete.");
    }
}

func main() {
    run_benchmark();
}
"#.to_string(),
                },
                TemplateFile {
                    path: ".gitignore".to_string(),
                    content: "# Basir / Roze build artifacts\n/target/\n/bin/\n*.class\n*.o\n*.log\n__roze_*\n".to_string(),
                },
                TemplateFile {
                    path: "README.md".to_string(),
                    content: vec![
                        "# {{project_name}}",
                        "",
                        "A Roze project created with `bsr new`.",
                        "",
                        "## Build",
                        "```bash",
                        "bsr build",
                        "```",
                        "",
                        "## Run",
                        "```bash",
                        "bsr run",
                        "```",
                        "",
                        "## Test",
                        "```bash",
                        "bsr test",
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
                    path: "bsr.mfz".to_string(),
                    content: r#"# bsr.mfz - Basir Project Manifest (Mohammad Firoze)
name = "{{project_name}}"
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
                        "bsr run",
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
                    path: "bsr.mfz".to_string(),
                    content: r#"# bsr.mfz - Basir Project Manifest (Mohammad Firoze)
name = "{{project_name}}"
version = "0.1.0"
edition = "2026"
description = "A Roze library"
authors = ["{{author}}"]
main = "src/lib.roze"
"#.to_string(),
                },
                TemplateFile {
                    path: "tests/integration_test.roze".to_string(),
                    content: r#"// tests/integration_test.roze - Integration tests for {{project_name}}

func assert_eq_int(actual: int, expected: int, msg: string) {
    if actual != expected {
        println("❌ Assertion failed: " + msg);
    } else {
        println("✅ " + msg);
    }
}

func test_basic_arithmetic() {
    let result = 1 + 1;
    assert_eq_int(result, 2, "1 + 1 == 2");
}

func main() {
    println("🧪 Running integration tests for {{project_name}}...");
    test_basic_arithmetic();
    println("All integration tests passed!");
}
"#.to_string(),
                },
                TemplateFile {
                    path: "benches/bench_main.roze".to_string(),
                    content: r#"// benches/bench_main.roze - Benchmark harness for {{project_name}}

func run_benchmark() {
    println("⏱️  Running benchmarks for {{project_name}}...");
    let sum = 0;
    let i = 0;
    while i < 100000 {
        sum = sum + i;
        i = i + 1;
    }
    if sum > 0 {
        println("✅ Benchmark iterations complete.");
    }
}

func main() {
    run_benchmark();
}
"#.to_string(),
                },
                TemplateFile {
                    path: ".gitignore".to_string(),
                    content: "# Basir / Roze build artifacts\n/target/\n/bin/\n*.class\n*.o\n*.log\n__roze_*\n".to_string(),
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