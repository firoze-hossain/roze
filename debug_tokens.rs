use std::fs;
use roze_compiler::lexer::tokenize;

fn main() {
    let source = fs::read_to_string("test_simple.roze").unwrap_or_else(|_| {
        r#"
func main() {
    println("Hello from Roze!");
}
"#.to_string()
    });

    println!("Source code:");
    println!("{}", source);
    println!("\n--- Tokens ---");

    let tokens = tokenize(&source);
    for (i, token) in tokens.iter().enumerate() {
        println!("Token {:2}: {:?} at line {} col {}",
                 i, token.token, token.line, token.column);
    }
}