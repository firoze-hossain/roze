use crate::parser::ast::*;
use anyhow::Result;
use std::collections::HashMap;

/// Java type used for Roze's "no annotation given" / dynamic-ish values.
const OBJECT_TYPE: &str = "Object";

/// Fixed helper methods emitted into every generated class, backing the
/// file/network intrinsics. Java requires checked exceptions
/// (`IOException`, `InterruptedException`) to be caught or declared;
/// Roze has no `throws` syntax and generated methods never declare any,
/// so these wrap each checked-exception-throwing call once here and
/// rethrow as an unchecked `RuntimeException` -- callers just see a
/// plain method call with no exception-handling of their own to write,
/// consistent with Roze not having a Result/Option type yet (see
/// ROADMAP.md).
const IO_HELPER_METHODS: &str = r#"
    private static String __roze_read_file(String path) {
        try {
            return new String(java.nio.file.Files.readAllBytes(java.nio.file.Paths.get(path)));
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        }
    }

    private static void __roze_write_file(String path, String content) {
        try {
            java.nio.file.Files.write(java.nio.file.Paths.get(path), content.getBytes());
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        }
    }

    private static void __roze_append_file(String path, String content) {
        try {
            java.nio.file.Files.write(
                java.nio.file.Paths.get(path),
                content.getBytes(),
                java.nio.file.StandardOpenOption.CREATE,
                java.nio.file.StandardOpenOption.APPEND
            );
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        }
    }

    private static boolean __roze_delete_file(String path) {
        try {
            return java.nio.file.Files.deleteIfExists(java.nio.file.Paths.get(path));
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        }
    }

    private static java.util.List __roze_read_lines(String path) {
        try {
            return new java.util.ArrayList(java.nio.file.Files.readAllLines(java.nio.file.Paths.get(path)));
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        }
    }

    private static String __roze_http_get(String url) {
        try {
            java.net.http.HttpClient client = java.net.http.HttpClient.newHttpClient();
            java.net.http.HttpRequest request = java.net.http.HttpRequest.newBuilder()
                .uri(java.net.URI.create(url))
                .GET()
                .build();
            java.net.http.HttpResponse<String> response = client.send(request, java.net.http.HttpResponse.BodyHandlers.ofString());
            return response.body();
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new RuntimeException(e);
        }
    }

    private static String __roze_http_post(String url, String body) {
        try {
            java.net.http.HttpClient client = java.net.http.HttpClient.newHttpClient();
            java.net.http.HttpRequest request = java.net.http.HttpRequest.newBuilder()
                .uri(java.net.URI.create(url))
                .header("Content-Type", "application/json")
                .POST(java.net.http.HttpRequest.BodyPublishers.ofString(body))
                .build();
            java.net.http.HttpResponse<String> response = client.send(request, java.net.http.HttpResponse.BodyHandlers.ofString());
            return response.body();
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new RuntimeException(e);
        }
    }
"#;

/// JSON encode/decode helpers. There is no JSON support in the standard
/// JDK at all (not even an API, unlike java.sql or java.net.http) --
/// every JVM program either hand-rolls one or pulls in a third-party
/// library, and Roze has no dependency/classpath story for the latter
/// (see the SQL helpers below for where that does become unavoidable).
/// A minimal recursive-descent parser and matching encoder is a small
/// enough, well-bounded thing to embed directly.
///
/// Encoding maps Roze's untyped values onto JSON in the obvious way:
/// String -> JSON string, Boolean -> true/false, any Number -> a JSON
/// number, `list` -> JSON array, `map` -> JSON object (keys coerced to
/// strings via toString, same as JSON itself requires), null -> null,
/// anything else -> its toString() as a JSON string.
///
/// Decoding produces the same shapes back: JSON objects/arrays become
/// `map`/`list` (so json_decode's result composes directly with the
/// existing map_get/list_get intrinsics), JSON strings/booleans/null
/// become String/Boolean/null, and JSON numbers become Integer/Long/
/// Double depending on shape -- Roze has no float type, so a decoded
/// float is an Unknown-typed Double, usable via to_string but not via
/// int-specific intrinsics like abs/max/min.
const JSON_HELPER_METHODS: &str = r#"
    private static String __roze_json_encode(Object value) {
        StringBuilder sb = new StringBuilder();
        __roze_json_encode_value(value, sb);
        return sb.toString();
    }

    private static void __roze_json_encode_value(Object value, StringBuilder sb) {
        if (value == null) {
            sb.append("null");
        } else if (value instanceof String) {
            __roze_json_encode_string((String) value, sb);
        } else if (value instanceof Boolean || value instanceof Integer || value instanceof Long || value instanceof Double || value instanceof Float) {
            sb.append(value.toString());
        } else if (value instanceof java.util.List) {
            sb.append('[');
            java.util.List<?> list = (java.util.List<?>) value;
            for (int i = 0; i < list.size(); i++) {
                if (i > 0) sb.append(',');
                __roze_json_encode_value(list.get(i), sb);
            }
            sb.append(']');
        } else if (value instanceof java.util.Map) {
            sb.append('{');
            java.util.Map<?, ?> map = (java.util.Map<?, ?>) value;
            boolean first = true;
            for (java.util.Map.Entry<?, ?> entry : map.entrySet()) {
                if (!first) sb.append(',');
                first = false;
                __roze_json_encode_string(String.valueOf(entry.getKey()), sb);
                sb.append(':');
                __roze_json_encode_value(entry.getValue(), sb);
            }
            sb.append('}');
        } else {
            __roze_json_encode_string(value.toString(), sb);
        }
    }

    private static void __roze_json_encode_string(String s, StringBuilder sb) {
        sb.append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c == '"') {
                sb.append("\\\"");
            } else if (c == '\\') {
                sb.append("\\\\");
            } else if (c == '\n') {
                sb.append("\\n");
            } else if (c == '\r') {
                sb.append("\\r");
            } else if (c == '\t') {
                sb.append("\\t");
            } else if (c < 0x20) {
                sb.append(String.format("\\u%04x", (int) c));
            } else {
                sb.append(c);
            }
        }
        sb.append('"');
    }

    private static Object __roze_json_decode(String json) {
        int[] pos = new int[]{0};
        return __roze_json_parse_value(json, pos);
    }

    private static void __roze_json_skip_ws(String s, int[] pos) {
        while (pos[0] < s.length() && Character.isWhitespace(s.charAt(pos[0]))) pos[0]++;
    }

    private static Object __roze_json_parse_value(String s, int[] pos) {
        __roze_json_skip_ws(s, pos);
        char c = s.charAt(pos[0]);
        if (c == '{') return __roze_json_parse_object(s, pos);
        if (c == '[') return __roze_json_parse_array(s, pos);
        if (c == '"') return __roze_json_parse_string(s, pos);
        if (c == 't') { pos[0] += 4; return Boolean.TRUE; }
        if (c == 'f') { pos[0] += 5; return Boolean.FALSE; }
        if (c == 'n') { pos[0] += 4; return null; }
        return __roze_json_parse_number(s, pos);
    }

    private static java.util.Map __roze_json_parse_object(String s, int[] pos) {
        java.util.Map<Object, Object> map = new java.util.HashMap<Object, Object>();
        pos[0]++;
        __roze_json_skip_ws(s, pos);
        if (s.charAt(pos[0]) == '}') { pos[0]++; return map; }
        while (true) {
            __roze_json_skip_ws(s, pos);
            String key = __roze_json_parse_string(s, pos);
            __roze_json_skip_ws(s, pos);
            pos[0]++;
            Object value = __roze_json_parse_value(s, pos);
            map.put(key, value);
            __roze_json_skip_ws(s, pos);
            char c = s.charAt(pos[0]);
            if (c == ',') { pos[0]++; continue; }
            if (c == '}') { pos[0]++; break; }
            throw new RuntimeException("Invalid JSON: expected ',' or '}' at position " + pos[0]);
        }
        return map;
    }

    private static java.util.List __roze_json_parse_array(String s, int[] pos) {
        java.util.List<Object> list = new java.util.ArrayList<Object>();
        pos[0]++;
        __roze_json_skip_ws(s, pos);
        if (s.charAt(pos[0]) == ']') { pos[0]++; return list; }
        while (true) {
            Object value = __roze_json_parse_value(s, pos);
            list.add(value);
            __roze_json_skip_ws(s, pos);
            char c = s.charAt(pos[0]);
            if (c == ',') { pos[0]++; continue; }
            if (c == ']') { pos[0]++; break; }
            throw new RuntimeException("Invalid JSON: expected ',' or ']' at position " + pos[0]);
        }
        return list;
    }

    private static String __roze_json_parse_string(String s, int[] pos) {
        pos[0]++;
        StringBuilder sb = new StringBuilder();
        while (s.charAt(pos[0]) != '"') {
            char c = s.charAt(pos[0]);
            if (c == '\\') {
                pos[0]++;
                char esc = s.charAt(pos[0]);
                if (esc == '"') sb.append('"');
                else if (esc == '\\') sb.append('\\');
                else if (esc == '/') sb.append('/');
                else if (esc == 'n') sb.append('\n');
                else if (esc == 't') sb.append('\t');
                else if (esc == 'r') sb.append('\r');
                else if (esc == 'b') sb.append('\b');
                else if (esc == 'f') sb.append('\f');
                else if (esc == 'u') {
                    String hex = s.substring(pos[0] + 1, pos[0] + 5);
                    sb.append((char) Integer.parseInt(hex, 16));
                    pos[0] += 4;
                } else {
                    sb.append(esc);
                }
                pos[0]++;
            } else {
                sb.append(c);
                pos[0]++;
            }
        }
        pos[0]++;
        return sb.toString();
    }

    private static Object __roze_json_parse_number(String s, int[] pos) {
        int start = pos[0];
        if (s.charAt(pos[0]) == '-') pos[0]++;
        while (pos[0] < s.length() && Character.isDigit(s.charAt(pos[0]))) pos[0]++;
        boolean isDouble = false;
        if (pos[0] < s.length() && s.charAt(pos[0]) == '.') {
            isDouble = true;
            pos[0]++;
            while (pos[0] < s.length() && Character.isDigit(s.charAt(pos[0]))) pos[0]++;
        }
        if (pos[0] < s.length() && (s.charAt(pos[0]) == 'e' || s.charAt(pos[0]) == 'E')) {
            isDouble = true;
            pos[0]++;
            if (pos[0] < s.length() && (s.charAt(pos[0]) == '+' || s.charAt(pos[0]) == '-')) pos[0]++;
            while (pos[0] < s.length() && Character.isDigit(s.charAt(pos[0]))) pos[0]++;
        }
        String numStr = s.substring(start, pos[0]);
        if (isDouble) {
            return Double.parseDouble(numStr);
        }
        try {
            return Integer.parseInt(numStr);
        } catch (NumberFormatException e) {
            return Long.parseLong(numStr);
        }
    }
"#;

/// A minimal, synchronous, single-connection-at-a-time HTTP server:
/// `http_server_start` binds a socket, `http_server_accept` blocks for
/// the next request and returns it as a plain `map` (so it composes
/// directly with the existing map_get/map_has intrinsics -- no new type
/// needed), and `http_server_respond` writes a response and closes that
/// connection. This shape exists specifically because Roze has no
/// closures/lambdas yet, so there's no way for user code to hand the
/// server a per-request callback the way frameworks like Spring Boot
/// do -- an explicit accept/respond loop the user writes themselves is
/// the pragmatic alternative. No concurrency: one request is fully
/// handled before the next is accepted. See stdlib/src/io.roze.
const HTTP_SERVER_HELPER_METHODS: &str = r#"
    private static java.net.ServerSocket __roze_http_server_start(int port) {
        try {
            return new java.net.ServerSocket(port);
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        }
    }

    private static int __roze_find_header_end(byte[] data) {
        for (int i = 0; i + 3 < data.length; i++) {
            if (data[i] == '\r' && data[i + 1] == '\n' && data[i + 2] == '\r' && data[i + 3] == '\n') {
                return i;
            }
        }
        return -1;
    }

    private static java.util.Map __roze_http_server_accept(java.net.ServerSocket server) {
        try {
            java.net.Socket socket = server.accept();
            java.io.InputStream in = socket.getInputStream();
            java.io.ByteArrayOutputStream buffer = new java.io.ByteArrayOutputStream();
            byte[] chunk = new byte[8192];
            int headerEnd = -1;
            while (headerEnd < 0) {
                int n = in.read(chunk);
                if (n <= 0) break;
                buffer.write(chunk, 0, n);
                headerEnd = __roze_find_header_end(buffer.toByteArray());
            }
            byte[] all = buffer.toByteArray();
            String headerText = new String(all, 0, Math.max(headerEnd, 0), "UTF-8");
            String[] lines = headerText.split("\r\n");
            String[] requestLineParts = lines.length > 0 ? lines[0].split(" ") : new String[0];
            String method = requestLineParts.length > 0 ? requestLineParts[0] : "GET";
            String path = requestLineParts.length > 1 ? requestLineParts[1] : "/";

            int contentLength = 0;
            for (String line : lines) {
                if (line.toLowerCase().startsWith("content-length:")) {
                    contentLength = Integer.parseInt(line.substring(line.indexOf(':') + 1).trim());
                }
            }

            int bodyStart = headerEnd + 4;
            java.io.ByteArrayOutputStream bodyBuffer = new java.io.ByteArrayOutputStream();
            if (bodyStart < all.length) {
                bodyBuffer.write(all, bodyStart, all.length - bodyStart);
            }
            while (bodyBuffer.size() < contentLength) {
                int n = in.read(chunk);
                if (n <= 0) break;
                bodyBuffer.write(chunk, 0, n);
            }
            String body = bodyBuffer.toString("UTF-8");

            java.util.Map<Object, Object> request = new java.util.HashMap<Object, Object>();
            request.put("method", method);
            request.put("path", path);
            request.put("body", body);
            request.put("__socket", socket);
            return request;
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        }
    }

    private static String __roze_http_status_text(int status) {
        if (status == 200) return "OK";
        if (status == 201) return "Created";
        if (status == 204) return "No Content";
        if (status == 301) return "Moved Permanently";
        if (status == 302) return "Found";
        if (status == 400) return "Bad Request";
        if (status == 401) return "Unauthorized";
        if (status == 403) return "Forbidden";
        if (status == 404) return "Not Found";
        if (status == 500) return "Internal Server Error";
        return "Unknown";
    }

    private static void __roze_http_server_respond(java.util.Map request, int status, String body) {
        try {
            java.net.Socket socket = (java.net.Socket) request.get("__socket");
            byte[] bodyBytes = body.getBytes("UTF-8");
            String header = "HTTP/1.1 " + status + " " + __roze_http_status_text(status) + "\r\n"
                + "Content-Type: text/plain; charset=utf-8\r\n"
                + "Content-Length: " + bodyBytes.length + "\r\n"
                + "Connection: close\r\n\r\n";
            java.io.OutputStream out = socket.getOutputStream();
            out.write(header.getBytes("UTF-8"));
            out.write(bodyBytes);
            out.flush();
            socket.close();
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        }
    }

    private static void __roze_http_server_stop(java.net.ServerSocket server) {
        try {
            server.close();
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        }
    }
"#;

/// SQL helpers, built on the JDK's own `java.sql` API (part of the
/// standard library since Java 1.1). Unlike every other intrinsic in
/// this file, these depend on something Roze can't provide itself: an
/// actual JDBC *driver* implementation for whatever database you want
/// to talk to. The JDK ships the java.sql interfaces only, never a
/// driver -- there is no such thing as a batteries-included database
/// connection in plain Java, for any database. `DriverManager` auto-
/// discovers whatever driver is on the classpath at runtime (JDBC 4+
/// drivers self-register via META-INF/services, so no explicit
/// `Class.forName(...)` is needed here), which is why `roze build`/
/// `roze run` gained a `--classpath` flag alongside this feature: point
/// it at a driver jar (H2, SQLite, Postgres, ...) and `sql_connect`
/// works with that database's URL scheme. See stdlib/src/sql.roze.
const SQL_HELPER_METHODS: &str = r#"
    private static java.sql.Connection __roze_sql_connect(String url) {
        try {
            return java.sql.DriverManager.getConnection(url);
        } catch (java.sql.SQLException e) {
            throw new RuntimeException(e);
        }
    }

    private static java.util.List __roze_sql_query(java.sql.Connection conn, String sql) {
        try {
            java.sql.Statement stmt = conn.createStatement();
            java.sql.ResultSet rs = stmt.executeQuery(sql);
            java.sql.ResultSetMetaData meta = rs.getMetaData();
            int columnCount = meta.getColumnCount();
            java.util.List<Object> rows = new java.util.ArrayList<Object>();
            while (rs.next()) {
                java.util.Map<Object, Object> row = new java.util.HashMap<Object, Object>();
                for (int i = 1; i <= columnCount; i++) {
                    row.put(meta.getColumnLabel(i), rs.getObject(i));
                }
                rows.add(row);
            }
            rs.close();
            stmt.close();
            return rows;
        } catch (java.sql.SQLException e) {
            throw new RuntimeException(e);
        }
    }

    private static int __roze_sql_execute(java.sql.Connection conn, String sql) {
        try {
            java.sql.Statement stmt = conn.createStatement();
            int affected = stmt.executeUpdate(sql);
            stmt.close();
            return affected;
        } catch (java.sql.SQLException e) {
            throw new RuntimeException(e);
        }
    }

    private static void __roze_sql_close(java.sql.Connection conn) {
        try {
            conn.close();
        } catch (java.sql.SQLException e) {
            throw new RuntimeException(e);
        }
    }
"#;

/// Returns the Java return type for a built-in Core (string/math)
/// intrinsic, or None if `name` isn't one. Keep this in sync with
/// `semantic::builtin_signatures`.
fn intrinsic_return_type(name: &str) -> Option<&'static str> {
    match name {
        "string_length" => Some("int"),
        "string_concat" => Some("String"),
        "string_to_upper" => Some("String"),
        "string_to_lower" => Some("String"),
        "abs" => Some("int"),
        "max" => Some("int"),
        "min" => Some("int"),
        "to_string" => Some("String"),
        "to_int" => Some("int"),
        "is_number" => Some("boolean"),
        "is_string" => Some("boolean"),

        "list_new" => Some("java.util.List"),
        "list_push" => Some("boolean"),
        "list_get" => Some(OBJECT_TYPE),
        "list_set" => Some(OBJECT_TYPE),
        "list_remove" => Some(OBJECT_TYPE),
        "list_length" => Some("int"),
        "list_is_empty" => Some("boolean"),

        "map_new" => Some("java.util.Map"),
        "map_put" => Some(OBJECT_TYPE),
        "map_get" => Some(OBJECT_TYPE),
        "map_has" => Some("boolean"),
        "map_remove" => Some(OBJECT_TYPE),
        "map_size" => Some("int"),
        "map_is_empty" => Some("boolean"),

        "read_file" => Some("String"),
        "write_file" => Some("void"),
        "append_file" => Some("void"),
        "file_exists" => Some("boolean"),
        "delete_file" => Some("boolean"),
        "read_lines" => Some("java.util.List"),

        "http_get" => Some("String"),
        "http_post" => Some("String"),

        "json_encode" => Some("String"),
        "json_decode" => Some(OBJECT_TYPE),

        "http_server_start" => Some("java.net.ServerSocket"),
        "http_server_accept" => Some("java.util.Map"),
        "http_server_respond" => Some("void"),
        "http_server_stop" => Some("void"),

        "sql_connect" => Some(OBJECT_TYPE),
        "sql_query" => Some("java.util.List"),
        "sql_execute" => Some("int"),
        "sql_close" => Some("void"),

        _ => None,
    }
}

fn is_intrinsic(name: &str) -> bool {
    intrinsic_return_type(name).is_some()
}

/// Escapes a raw string value (already fully resolved by the Roze
/// lexer -- e.g. a literal `\n` in the source is, by this point, a real
/// newline byte, not the two characters backslash-n) back into the
/// contents of a Java string literal. Must handle every control
/// character the lexer can produce, not just backslash/quote: emitting a
/// raw newline (or tab, or carriage return) directly into generated
/// Java source text produces invalid Java ("unclosed string literal"),
/// since Java string literals can't contain a literal newline.
fn escape_for_java_string_literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),  // backspace
            '\u{c}' => out.push_str("\\f"),  // form feed
            _ => out.push(c),
        }
    }
    out
}

/// Maps a Roze source-level type annotation to a Java type name.
/// `default_java` is used when no annotation was given at all (distinct
/// from an annotation the compiler doesn't recognize, which still maps to
/// Object).
fn roze_type_to_java(type_name: Option<&str>, default_java: &str) -> String {
    match type_name {
        Some("int") => "int".to_string(),
        Some("string") => "String".to_string(),
        Some("bool") => "boolean".to_string(),
        Some("void") => "void".to_string(),
        Some("list") | Some("List") => "java.util.List".to_string(),
        Some("map") | Some("Map") => "java.util.Map".to_string(),
        Some(_) => OBJECT_TYPE.to_string(),
        None => default_java.to_string(),
    }
}

/// name -> java type, for locals/params visible at the current point in
/// generation. A `Vec` of frames acts as a scope stack (innermost last).
type Scope = Vec<HashMap<String, String>>;

pub struct JavaSourceGenerator {
    program: Program,
    class_name: String,
    /// name -> (java param types, java return type) for every user-defined
    /// top-level function, so call sites and `let` type inference never
    /// have to guess.
    functions: HashMap<String, (Vec<String>, String)>,
}

impl JavaSourceGenerator {
    pub fn new(program: Program, class_name: String) -> Self {
        let mut functions = HashMap::new();
        for stmt in &program.statements {
            if let Statement::Function { name, params, return_type, .. } = stmt {
                let param_types: Vec<String> = params.iter()
                    .map(|p| roze_type_to_java(p.type_name.as_deref(), OBJECT_TYPE))
                    .collect();
                let ret = roze_type_to_java(return_type.as_deref(), "void");
                functions.insert(name.clone(), (param_types, ret));
            }
        }
        Self { program, class_name, functions }
    }

    pub fn generate(&self) -> Result<String> {
        let mut source = String::new();
        source.push_str("// Generated by Roze compiler v0.1\n");
        source.push_str(&format!("public class {} {{\n", self.class_name));

        let mut main_found = false;
        for stmt in &self.program.statements {
            if let Statement::Function { name, params, body, .. } = stmt {
                // Core intrinsics are handled entirely at each call site
                // (see generate_call) and never emitted as a real method,
                // even if a same-named function happens to be declared.
                if is_intrinsic(name) {
                    continue;
                }

                if name == "main" {
                    source.push_str("    public static void main(String[] args) {\n");
                    let mut scope: Scope = vec![HashMap::new()];
                    self.generate_statement(&mut source, body, 2, &mut scope)?;
                    source.push_str("    }\n");
                    main_found = true;
                } else {
                    let (param_types, ret_java) = self.functions.get(name).cloned().unwrap_or_default();
                    let params_sig: Vec<String> = params.iter().zip(param_types.iter())
                        .map(|(p, t)| format!("{} {}", t, p.name))
                        .collect();

                    source.push_str(&format!(
                        "    public static {} {}({}) {{\n",
                        ret_java, name, params_sig.join(", ")
                    ));

                    let mut scope: Scope = vec![HashMap::new()];
                    {
                        let frame = scope.last_mut().expect("scope always has a frame");
                        for (p, t) in params.iter().zip(param_types.iter()) {
                            frame.insert(p.name.clone(), t.clone());
                        }
                    }
                    self.generate_statement(&mut source, body, 2, &mut scope)?;
                    source.push_str("    }\n");
                }
            }
        }

        if !main_found {
            source.push_str("    public static void main(String[] args) {\n");
            source.push_str("        System.out.println(\"No main function found!\");\n");
            source.push_str("    }\n");
        }

        source.push_str(IO_HELPER_METHODS);
        source.push_str(JSON_HELPER_METHODS);
        source.push_str(HTTP_SERVER_HELPER_METHODS);
        source.push_str(SQL_HELPER_METHODS);
        source.push_str("}\n");

        Ok(source)
    }

    fn generate_statement(&self, source: &mut String, stmt: &Statement, indent: usize, scope: &mut Scope) -> Result<()> {
        let indent_str = "    ".repeat(indent);

        match stmt {
            Statement::Block { statements, .. } => {
                for stmt in statements {
                    self.generate_statement(source, stmt, indent, scope)?;
                }
            }
            Statement::Expression { expr, .. } => {
                if let Expression::Call { function, arguments, .. } = expr.as_ref() {
                    if let Expression::Identifier { name, .. } = function.as_ref() {
                        if name == "println" {
                            source.push_str(&format!("{}System.out.println(", indent_str));
                            self.generate_println_args(source, arguments, scope)?;
                            source.push_str(");\n");
                            return Ok(());
                        }
                    }
                }
                source.push_str(&indent_str);
                self.generate_expression(source, expr, scope)?;
                source.push_str(";\n");
            }
            Statement::Let { name, value, .. } => {
                let java_type = self.infer_type(value, scope);

                source.push_str(&format!("{}{} {} = ", indent_str, java_type, name));
                self.generate_expression(source, value, scope)?;
                source.push_str(";\n");

                scope.last_mut().expect("scope always has a frame").insert(name.clone(), java_type);
            }
            Statement::Return { value, .. } => {
                if let Some(expr) = value {
                    source.push_str(&format!("{}return ", indent_str));
                    self.generate_expression(source, expr, scope)?;
                    source.push_str(";\n");
                } else {
                    source.push_str(&format!("{}return;\n", indent_str));
                }
            }
            Statement::Function { .. } => {
                // Nested function declarations aren't supported; top-level
                // functions are collected and emitted directly in `generate`.
            }
            Statement::Import { .. } => {
                // Imports are already resolved into real functions before
                // this stage even runs (see imports::resolve_imports),
                // so in practice there's nothing left to do here -- this
                // arm only exists in case that ever changes.
            }
            Statement::Assign { name, value, .. } => {
                source.push_str(&format!("{}{} = ", indent_str, name));
                self.generate_expression(source, value, scope)?;
                source.push_str(";\n");
            }
            Statement::If { condition, then_branch, else_branch, .. } => {
                self.generate_if_chain(source, condition, then_branch, else_branch.as_deref(), indent, scope, true)?;
            }
            Statement::While { condition, body, .. } => {
                source.push_str(&format!("{}while (", indent_str));
                self.generate_expression(source, condition, scope)?;
                source.push_str(") {\n");
                scope.push(HashMap::new());
                self.generate_statement(source, body, indent + 1, scope)?;
                scope.pop();
                source.push_str(&format!("{}}}\n", indent_str));
            }
            Statement::For { init, condition, update, body, .. } => {
                source.push_str(&format!("{}for (", indent_str));
                scope.push(HashMap::new());
                self.generate_for_clause(source, init, scope)?;
                source.push_str("; ");
                self.generate_expression(source, condition, scope)?;
                source.push_str("; ");
                self.generate_for_clause(source, update, scope)?;
                source.push_str(") {\n");
                self.generate_statement(source, body, indent + 1, scope)?;
                scope.pop();
                source.push_str(&format!("{}}}\n", indent_str));
            }
        }

        Ok(())
    }

    /// Renders a for-loop's init or update clause inline inside the
    /// Java `for (...)` header -- i.e. like `generate_statement`'s
    /// `Let`/`Assign` cases, but without the indent prefix or trailing
    /// `;\n` those emit as full statements. The parser guarantees a
    /// for-loop's init/update are always `Let` or `Assign`.
    fn generate_for_clause(&self, source: &mut String, stmt: &Statement, scope: &mut Scope) -> Result<()> {
        match stmt {
            Statement::Let { name, value, .. } => {
                let java_type = self.infer_type(value, scope);
                source.push_str(&format!("{} {} = ", java_type, name));
                self.generate_expression(source, value, scope)?;
                scope.last_mut().expect("scope always has a frame").insert(name.clone(), java_type);
            }
            Statement::Assign { name, value, .. } => {
                source.push_str(&format!("{} = ", name));
                self.generate_expression(source, value, scope)?;
            }
            _ => unreachable!("for-loop init/update is always Let or Assign (enforced by the parser)"),
        }
        Ok(())
    }

    /// Emits `if (cond) { ... }`, followed by any `else` / `else if` chain,
    /// ending with exactly one trailing newline.
    fn generate_if_chain(
        &self,
        source: &mut String,
        condition: &Expression,
        then_branch: &Statement,
        else_branch: Option<&Statement>,
        indent: usize,
        scope: &mut Scope,
        write_indent: bool,
    ) -> Result<()> {
        let indent_str = "    ".repeat(indent);

        if write_indent {
            source.push_str(&indent_str);
        }
        source.push_str("if (");
        self.generate_expression(source, condition, scope)?;
        source.push_str(") {\n");
        scope.push(HashMap::new());
        self.generate_statement(source, then_branch, indent + 1, scope)?;
        scope.pop();
        source.push_str(&format!("{}}}", indent_str));

        match else_branch {
            None => source.push('\n'),
            Some(Statement::If { condition, then_branch, else_branch, .. }) => {
                source.push_str(" else ");
                self.generate_if_chain(source, condition, then_branch, else_branch.as_deref(), indent, scope, false)?;
            }
            Some(other) => {
                source.push_str(" else {\n");
                scope.push(HashMap::new());
                self.generate_statement(source, other, indent + 1, scope)?;
                scope.pop();
                source.push_str(&format!("{}}}\n", indent_str));
            }
        }

        Ok(())
    }

    fn generate_println_args(&self, source: &mut String, arguments: &[Expression], scope: &Scope) -> Result<()> {
        if arguments.is_empty() {
            return Ok(());
        }
        if arguments.len() == 1 {
            return self.generate_expression(source, &arguments[0], scope);
        }
        // println with several arguments concatenates them, so
        // `println(a, b)` behaves like `println(a + b)`.
        source.push('(');
        for (i, arg) in arguments.iter().enumerate() {
            if i > 0 {
                source.push_str(" + ");
            }
            self.generate_expression(source, arg, scope)?;
        }
        source.push(')');
        Ok(())
    }

    /// Infers the Java type an expression will evaluate to, using tracked
    /// local/parameter types instead of guessing from variable names.
    fn infer_type(&self, expr: &Expression, scope: &Scope) -> String {
        match expr {
            Expression::Number { .. } => "int".to_string(),
            Expression::String { .. } => "String".to_string(),
            Expression::Boolean { .. } => "boolean".to_string(),
            Expression::Null { .. } => OBJECT_TYPE.to_string(),
            Expression::Identifier { name, .. } => {
                for frame in scope.iter().rev() {
                    if let Some(t) = frame.get(name) {
                        return t.clone();
                    }
                }
                OBJECT_TYPE.to_string()
            }
            Expression::Unary { operand, .. } => self.infer_type(operand, scope),
            Expression::Binary { left, operator, right, .. } => {
                let left_type = self.infer_type(left, scope);
                let right_type = self.infer_type(right, scope);

                match operator {
                    BinaryOperator::Add => {
                        if left_type == "String" || right_type == "String" {
                            "String".to_string()
                        } else if left_type == "int" && right_type == "int" {
                            "int".to_string()
                        } else {
                            // One side is some Object-typed/unknown value;
                            // Java's `+` between an Object and anything
                            // else always means string concatenation, so
                            // this is the only type that's always valid.
                            "String".to_string()
                        }
                    }
                    BinaryOperator::Subtract | BinaryOperator::Multiply | BinaryOperator::Divide => "int".to_string(),
                    BinaryOperator::Equal | BinaryOperator::NotEqual |
                    BinaryOperator::LessThan | BinaryOperator::GreaterThan |
                    BinaryOperator::LessEqual | BinaryOperator::GreaterEqual |
                    BinaryOperator::And | BinaryOperator::Or => "boolean".to_string(),
                }
            }
            Expression::Call { function, .. } => {
                if let Expression::Identifier { name, .. } = function.as_ref() {
                    if name == "println" {
                        return "void".to_string();
                    }
                    if let Some(ret) = intrinsic_return_type(name) {
                        return ret.to_string();
                    }
                    if let Some((_, ret)) = self.functions.get(name) {
                        return ret.clone();
                    }
                }
                OBJECT_TYPE.to_string()
            }
        }
    }

    fn generate_expression(&self, source: &mut String, expr: &Expression, scope: &Scope) -> Result<()> {
        match expr {
            Expression::String { value, .. } => {
                source.push_str(&format!("\"{}\"", escape_for_java_string_literal(value)));
            }
            Expression::Number { value, .. } => {
                source.push_str(value);
            }
            Expression::Identifier { name, .. } => {
                source.push_str(name);
            }
            Expression::Boolean { value, .. } => {
                source.push_str(&format!("{}", value));
            }
            Expression::Null { .. } => {
                source.push_str("null");
            }
            Expression::Call { function, arguments, .. } => {
                if let Expression::Identifier { name, .. } = function.as_ref() {
                    self.generate_call(source, name, arguments, scope)?;
                }
            }
            Expression::Binary { left, operator, right, .. } => {
                let left_type = self.infer_type(left, scope);
                let right_type = self.infer_type(right, scope);
                let is_primitive = |t: &str| t == "int" || t == "boolean";

                let use_equals_method = matches!(operator, BinaryOperator::Equal | BinaryOperator::NotEqual)
                    && (!is_primitive(&left_type) || !is_primitive(&right_type));

                if use_equals_method {
                    // Java's `==`/`!=` on a non-primitive type is
                    // reference identity, not content equality -- two
                    // distinct String objects with identical content
                    // (e.g. the result of runtime concatenation vs. a
                    // literal) would silently compare unequal. Route
                    // through Objects.equals instead, which is
                    // content-based and null-safe.
                    if matches!(operator, BinaryOperator::NotEqual) {
                        source.push('!');
                    }
                    source.push_str("java.util.Objects.equals(");
                    self.generate_expression(source, left, scope)?;
                    source.push_str(", ");
                    self.generate_expression(source, right, scope)?;
                    source.push(')');
                } else {
                    let is_string_concat = matches!(operator, BinaryOperator::Add)
                        && (left_type == "String" || right_type == "String");

                    source.push('(');
                    self.generate_expression(source, left, scope)?;
                    if is_string_concat {
                        source.push_str(" + ");
                    } else {
                        let op = match operator {
                            BinaryOperator::Add => " + ",
                            BinaryOperator::Subtract => " - ",
                            BinaryOperator::Multiply => " * ",
                            BinaryOperator::Divide => " / ",
                            BinaryOperator::Equal => " == ",
                            BinaryOperator::NotEqual => " != ",
                            BinaryOperator::LessThan => " < ",
                            BinaryOperator::GreaterThan => " > ",
                            BinaryOperator::LessEqual => " <= ",
                            BinaryOperator::GreaterEqual => " >= ",
                            BinaryOperator::And => " && ",
                            BinaryOperator::Or => " || ",
                        };
                        source.push_str(op);
                    }
                    self.generate_expression(source, right, scope)?;
                    source.push(')');
                }
            }
            Expression::Unary { operator, operand, .. } => {
                let op = match operator {
                    UnaryOperator::Negate => "-",
                    UnaryOperator::Not => "!",
                };
                source.push_str(op);
                self.generate_expression(source, operand, scope)?;
            }
        }

        Ok(())
    }

    /// Generates a call, rewriting Core (string/math) intrinsics to real
    /// JVM standard-library calls, since Roze has no method-call syntax of
    /// its own yet to express `s.length()` directly.
    fn generate_call(&self, source: &mut String, name: &str, arguments: &[Expression], scope: &Scope) -> Result<()> {
        match name {
            "string_length" if arguments.len() == 1 => {
                source.push_str("((String)(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(")).length()");
            }
            "string_to_upper" if arguments.len() == 1 => {
                source.push_str("((String)(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(")).toUpperCase()");
            }
            "string_to_lower" if arguments.len() == 1 => {
                source.push_str("((String)(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(")).toLowerCase()");
            }
            "string_concat" if arguments.len() == 2 => {
                source.push('(');
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(" + ");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push(')');
            }
            "abs" if arguments.len() == 1 => {
                source.push_str("Math.abs(");
                self.generate_int_arg(source, &arguments[0], scope)?;
                source.push(')');
            }
            "max" if arguments.len() == 2 => {
                source.push_str("Math.max(");
                self.generate_int_arg(source, &arguments[0], scope)?;
                source.push_str(", ");
                self.generate_int_arg(source, &arguments[1], scope)?;
                source.push(')');
            }
            "min" if arguments.len() == 2 => {
                source.push_str("Math.min(");
                self.generate_int_arg(source, &arguments[0], scope)?;
                source.push_str(", ");
                self.generate_int_arg(source, &arguments[1], scope)?;
                source.push(')');
            }
            "to_string" if arguments.len() == 1 => {
                source.push_str("String.valueOf(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push(')');
            }
            "to_int" if arguments.len() == 1 => {
                source.push_str("Integer.parseInt(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push(')');
            }
            "is_number" if arguments.len() == 1 => {
                source.push_str("((Object)(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(") instanceof Integer)");
            }
            "is_string" if arguments.len() == 1 => {
                source.push_str("((Object)(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(") instanceof String)");
            }

            // ---- Collections: List ----
            "list_new" if arguments.is_empty() => {
                source.push_str("new java.util.ArrayList()");
            }
            "list_push" if arguments.len() == 2 => {
                self.generate_list_receiver(source, &arguments[0], scope)?;
                source.push_str(".add(");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push(')');
            }
            "list_get" if arguments.len() == 2 => {
                self.generate_list_receiver(source, &arguments[0], scope)?;
                source.push_str(".get(");
                self.generate_index_arg(source, &arguments[1], scope)?;
                source.push(')');
            }
            "list_set" if arguments.len() == 3 => {
                self.generate_list_receiver(source, &arguments[0], scope)?;
                source.push_str(".set(");
                self.generate_index_arg(source, &arguments[1], scope)?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2], scope)?;
                source.push(')');
            }
            "list_remove" if arguments.len() == 2 => {
                // Must be a genuine primitive int (see generate_index_arg):
                // List.remove has both remove(int) and remove(Object)
                // overloads, and a boxed Integer argument binds to
                // remove(Object) -- remove-by-equality, not by index.
                self.generate_list_receiver(source, &arguments[0], scope)?;
                source.push_str(".remove(");
                self.generate_index_arg(source, &arguments[1], scope)?;
                source.push(')');
            }
            "list_length" if arguments.len() == 1 => {
                self.generate_list_receiver(source, &arguments[0], scope)?;
                source.push_str(".size()");
            }
            "list_is_empty" if arguments.len() == 1 => {
                self.generate_list_receiver(source, &arguments[0], scope)?;
                source.push_str(".isEmpty()");
            }

            // ---- Collections: Map ----
            "map_new" if arguments.is_empty() => {
                source.push_str("new java.util.HashMap()");
            }
            "map_put" if arguments.len() == 3 => {
                self.generate_map_receiver(source, &arguments[0], scope)?;
                source.push_str(".put(");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2], scope)?;
                source.push(')');
            }
            "map_get" if arguments.len() == 2 => {
                self.generate_map_receiver(source, &arguments[0], scope)?;
                source.push_str(".get(");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push(')');
            }
            "map_has" if arguments.len() == 2 => {
                self.generate_map_receiver(source, &arguments[0], scope)?;
                source.push_str(".containsKey(");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push(')');
            }
            "map_remove" if arguments.len() == 2 => {
                self.generate_map_receiver(source, &arguments[0], scope)?;
                source.push_str(".remove(");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push(')');
            }
            "map_size" if arguments.len() == 1 => {
                self.generate_map_receiver(source, &arguments[0], scope)?;
                source.push_str(".size()");
            }
            "map_is_empty" if arguments.len() == 1 => {
                self.generate_map_receiver(source, &arguments[0], scope)?;
                source.push_str(".isEmpty()");
            }

            // ---- IO: file ----
            // Each delegates to a fixed helper method (see
            // IO_HELPER_METHODS) that wraps the underlying checked
            // exception and rethrows unchecked.
            "read_file" if arguments.len() == 1 => {
                source.push_str("__roze_read_file(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push(')');
            }
            "write_file" if arguments.len() == 2 => {
                source.push_str("__roze_write_file(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push(')');
            }
            "append_file" if arguments.len() == 2 => {
                source.push_str("__roze_append_file(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push(')');
            }
            "file_exists" if arguments.len() == 1 => {
                // Files.exists doesn't throw, so no helper needed.
                source.push_str("java.nio.file.Files.exists(java.nio.file.Paths.get(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str("))");
            }
            "delete_file" if arguments.len() == 1 => {
                source.push_str("__roze_delete_file(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push(')');
            }
            "read_lines" if arguments.len() == 1 => {
                source.push_str("__roze_read_lines(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push(')');
            }

            // ---- IO: network ----
            "http_get" if arguments.len() == 1 => {
                source.push_str("__roze_http_get(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push(')');
            }
            "http_post" if arguments.len() == 2 => {
                source.push_str("__roze_http_post(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push(')');
            }

            // ---- Web: JSON ----
            "json_encode" if arguments.len() == 1 => {
                source.push_str("__roze_json_encode(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push(')');
            }
            "json_decode" if arguments.len() == 1 => {
                source.push_str("__roze_json_decode(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push(')');
            }

            // ---- Web: HTTP server ----
            "http_server_start" if arguments.len() == 1 => {
                source.push_str("__roze_http_server_start(");
                self.generate_int_arg(source, &arguments[0], scope)?;
                source.push(')');
            }
            "http_server_accept" if arguments.len() == 1 => {
                source.push_str("__roze_http_server_accept(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push(')');
            }
            "http_server_respond" if arguments.len() == 3 => {
                source.push_str("__roze_http_server_respond(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(", ");
                self.generate_int_arg(source, &arguments[1], scope)?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2], scope)?;
                source.push(')');
            }
            "http_server_stop" if arguments.len() == 1 => {
                source.push_str("__roze_http_server_stop(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push(')');
            }

            // ---- Database (SQL) ----
            "sql_connect" if arguments.len() == 1 => {
                source.push_str("__roze_sql_connect(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push(')');
            }
            "sql_query" if arguments.len() == 2 => {
                source.push_str("__roze_sql_query(((java.sql.Connection)(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(")), ");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push(')');
            }
            "sql_execute" if arguments.len() == 2 => {
                source.push_str("__roze_sql_execute(((java.sql.Connection)(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(")), ");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push(')');
            }
            "sql_close" if arguments.len() == 1 => {
                source.push_str("__roze_sql_close((java.sql.Connection)(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str("))");
            }

            "println" => {
                source.push_str("System.out.println(");
                self.generate_println_args(source, arguments, scope)?;
                source.push(')');
            }
            _ => {
                source.push_str(name);
                source.push('(');
                for (i, arg) in arguments.iter().enumerate() {
                    if i > 0 {
                        source.push_str(", ");
                    }
                    self.generate_expression(source, arg, scope)?;
                }
                source.push(')');
            }
        }
        Ok(())
    }

    /// Generates an expression known to be used in an `int`-typed position
    /// (a Math.* argument). If the expression's static type isn't already
    /// `int`, it's coming from an Object-typed (untyped-parameter) value,
    /// so we unbox it via Integer -- valid whenever the runtime value
    /// really is a boxed Integer, matching the declared `int` parameter.
    fn generate_int_arg(&self, source: &mut String, expr: &Expression, scope: &Scope) -> Result<()> {
        let t = self.infer_type(expr, scope);
        if t == "int" {
            self.generate_expression(source, expr, scope)
        } else {
            source.push_str("((Integer)(Object)(");
            self.generate_expression(source, expr, scope)?;
            source.push_str("))");
            Ok(())
        }
    }

    /// Generates an expression used as the receiver of a method call
    /// (e.g. the list in `list.add(x)`), wrapped in parens as a cheap,
    /// always-safe guard against needing to reason about whether the
    /// receiver expression needs them for any particular case.
    fn generate_receiver(&self, source: &mut String, expr: &Expression, scope: &Scope) -> Result<()> {
        source.push('(');
        self.generate_expression(source, expr, scope)?;
        source.push(')');
        Ok(())
    }

    /// Like `generate_receiver`, but for a value used as a `List`: casts
    /// to `java.util.List` unless the expression's static type is
    /// already known to be one. Needed because not every list-shaped
    /// value is statically typed `java.util.List` -- notably, anything
    /// coming out of `json_decode` is statically `Object` (its shape
    /// depends on the JSON text, not on anything the type checker can
    /// see), so `.get()`/`.size()`/etc need an explicit cast to resolve.
    fn generate_list_receiver(&self, source: &mut String, expr: &Expression, scope: &Scope) -> Result<()> {
        let t = self.infer_type(expr, scope);
        if t == "java.util.List" {
            self.generate_receiver(source, expr, scope)
        } else {
            source.push_str("((java.util.List)(");
            self.generate_expression(source, expr, scope)?;
            source.push_str("))");
            Ok(())
        }
    }

    /// Like `generate_list_receiver`, for `Map`.
    fn generate_map_receiver(&self, source: &mut String, expr: &Expression, scope: &Scope) -> Result<()> {
        let t = self.infer_type(expr, scope);
        if t == "java.util.Map" {
            self.generate_receiver(source, expr, scope)
        } else {
            source.push_str("((java.util.Map)(");
            self.generate_expression(source, expr, scope)?;
            source.push_str("))");
            Ok(())
        }
    }

    /// Like `generate_int_arg`, but guarantees a genuine primitive `int`
    /// (via an explicit `.intValue()` unboxing call) rather than
    /// potentially leaving a boxed `Integer`. Needed specifically for
    /// `List` index arguments: `List.remove` has both `remove(int)` and
    /// `remove(Object)` overloads, and a boxed `Integer` argument binds
    /// to `remove(Object)` (remove-by-equality) since that's reachable
    /// without any unboxing at all -- silently doing the wrong thing
    /// rather than failing to compile. `List.get`/`set` don't have that
    /// particular ambiguity, but using the same guaranteed-primitive
    /// helper for all three keeps index handling uniform.
    fn generate_index_arg(&self, source: &mut String, expr: &Expression, scope: &Scope) -> Result<()> {
        let t = self.infer_type(expr, scope);
        if t == "int" {
            self.generate_expression(source, expr, scope)
        } else {
            source.push_str("((Integer)(Object)(");
            self.generate_expression(source, expr, scope)?;
            source.push_str(")).intValue()");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn generate(src: &str) -> String {
        let program = parse(tokenize(src)).expect("fixture should parse");
        JavaSourceGenerator::new(program, "Fixture".to_string())
            .generate()
            .expect("fixture should generate")
    }

    #[test]
    fn emits_every_function_not_just_main() {
        let java = generate("func greet() { println(\"hi\"); } func main() { greet(); }");
        assert!(java.contains("static void greet"), "greet() should be emitted:\n{}", java);
        assert!(java.contains("static void main"), "main() should be emitted:\n{}", java);
        assert!(java.contains("greet();"), "main() should call greet():\n{}", java);
    }

    #[test]
    fn uses_declared_types_not_name_guessing() {
        // Previously, codegen guessed a variable's Java type from its
        // *name* (only x/y/z/i/j/k were ever treated as int). A variable
        // holding a string, named like one of those letters, must still
        // come out as String.
        let java = generate("func main() { let x = \"hello\"; println(x); }");
        assert!(java.contains("String x ="), "expected 'String x =', got:\n{}", java);
    }

    #[test]
    fn if_else_if_chain_generates_valid_shape() {
        let java = generate("func f(x: int) -> int { if x > 0 { return 1; } else if x < 0 { return -1; } else { return 0; } } func main() { }");
        assert!(java.contains("if (") && java.contains("} else if (") && java.contains("} else {"), "unexpected if/else shape:\n{}", java);
    }

    #[test]
    fn while_loop_generates_java_while() {
        let java = generate("func main() { let i = 0; while i < 3 { i = i + 1; } }");
        assert!(java.contains("while ("), "expected a while loop:\n{}", java);
    }

    #[test]
    fn for_loop_generates_java_for() {
        let java = generate("func main() { for let i = 0; i < 3; i = i + 1 { println(i); } }");
        assert!(java.contains("for (int i = 0;"), "expected a Java for-loop header:\n{}", java);
    }

    #[test]
    fn string_intrinsics_map_to_real_java_calls() {
        let java = generate("func main() { println(string_to_upper(\"hi\")); println(string_length(\"hi\")); }");
        assert!(java.contains(".toUpperCase()"), "expected toUpperCase():\n{}", java);
        assert!(java.contains(".length()"), "expected length():\n{}", java);
    }

    #[test]
    fn math_intrinsics_map_to_java_math() {
        let java = generate("func main() { println(abs(-5)); println(max(1, 2)); println(min(1, 2)); }");
        assert!(java.contains("Math.abs("));
        assert!(java.contains("Math.max("));
        assert!(java.contains("Math.min("));
    }

    #[test]
    fn intrinsic_named_function_is_never_emitted_as_a_real_method() {
        // Even if a program somehow declares its own `abs`, it must not
        // get emitted as a real Java method -- the call-site rewrite to
        // Math.abs() always takes priority.
        let java = generate("func abs(x: int) -> int { return x; } func main() { println(abs(-5)); }");
        assert!(!java.contains("static int abs"), "an 'abs' method should never be emitted:\n{}", java);
        assert!(java.contains("Math.abs("), "the call site should still use Math.abs():\n{}", java);
    }

    #[test]
    fn string_equality_compares_content_not_reference() {
        // Java's == on Strings is reference identity; a naive codegen
        // would emit `combined == "Hello World"` which silently gives the
        // wrong answer at runtime for a non-interned (e.g. runtime
        // concatenation) result. Must route through content equality.
        let java = generate("func main() { let a = \"Hello\"; let b = a + \"!\"; println(b == \"Hello!\"); }");
        assert!(java.contains("Objects.equals("), "expected content equality via Objects.equals:\n{}", java);
        assert!(!java.contains("== \"Hello!\""), "must not use reference equality (==) on strings:\n{}", java);
    }

    #[test]
    fn int_equality_still_uses_primitive_operator() {
        // No need to pay for Objects.equals boxing when both sides are
        // already primitives -- plain == is correct and cheaper.
        let java = generate("func main() { let a = 5; println(a == 5); }");
        assert!(java.contains("(a == 5)"), "expected plain == for int comparison:\n{}", java);
    }

    #[test]
    fn no_main_falls_back_to_placeholder() {
        let java = generate("func helper() { println(\"hi\"); }");
        assert!(java.contains("No main function found"));
    }

    // ---- Collections ----

    #[test]
    fn list_operations_map_to_java_list_methods() {
        let java = generate(
            "func main() { let l = list_new(); list_push(l, 1); println(list_get(l, 0)); list_set(l, 0, 2); list_remove(l, 0); println(list_length(l)); println(list_is_empty(l)); }"
        );
        assert!(java.contains("new java.util.ArrayList()"), "expected list_new -> ArrayList:\n{}", java);
        assert!(java.contains(").add("), "expected list_push -> .add(:\n{}", java);
        assert!(java.contains(").get("), "expected list_get -> .get(:\n{}", java);
        assert!(java.contains(").set("), "expected list_set -> .set(:\n{}", java);
        assert!(java.contains(").remove("), "expected list_remove -> .remove(:\n{}", java);
        assert!(java.contains(").size()"), "expected list_length -> .size():\n{}", java);
        assert!(java.contains(").isEmpty()"), "expected list_is_empty -> .isEmpty():\n{}", java);
    }

    #[test]
    fn list_remove_forces_primitive_int_to_avoid_overload_ambiguity() {
        // List.remove(int) vs remove(Object): an untyped (Object-typed)
        // index must be explicitly unboxed with .intValue(), or Java
        // silently picks remove(Object) -- remove-by-equality instead of
        // remove-by-index.
        let java = generate("func f(idx) { let l = list_new(); list_remove(l, idx); } func main() { }");
        assert!(java.contains(".intValue()"), "expected an explicit .intValue() unboxing for a non-int index:\n{}", java);
    }

    #[test]
    fn map_operations_map_to_java_map_methods() {
        let java = generate(
            "func main() { let m = map_new(); map_put(m, \"a\", 1); println(map_get(m, \"a\")); println(map_has(m, \"a\")); map_remove(m, \"a\"); println(map_size(m)); println(map_is_empty(m)); }"
        );
        assert!(java.contains("new java.util.HashMap()"), "expected map_new -> HashMap:\n{}", java);
        assert!(java.contains(").put("), "expected map_put -> .put(:\n{}", java);
        assert!(java.contains(").get("), "expected map_get -> .get(:\n{}", java);
        assert!(java.contains(").containsKey("), "expected map_has -> .containsKey(:\n{}", java);
        assert!(java.contains(").remove("), "expected map_remove -> .remove(:\n{}", java);
        assert!(java.contains(").size()"), "expected map_size -> .size():\n{}", java);
        assert!(java.contains(").isEmpty()"), "expected map_is_empty -> .isEmpty():\n{}", java);
    }

    #[test]
    fn list_and_map_declared_types_use_java_util() {
        let java = generate("func main() { let l = list_new(); let m = map_new(); }");
        assert!(java.contains("java.util.List l ="), "expected 'java.util.List l =':\n{}", java);
        assert!(java.contains("java.util.Map m ="), "expected 'java.util.Map m =':\n{}", java);
    }

    // ---- IO: file ----

    #[test]
    fn file_intrinsics_delegate_to_helper_methods() {
        let java = generate(
            "func main() { write_file(\"a\", \"b\"); append_file(\"a\", \"c\"); read_file(\"a\"); delete_file(\"a\"); read_lines(\"a\"); }"
        );
        assert!(java.contains("__roze_write_file("));
        assert!(java.contains("__roze_append_file("));
        assert!(java.contains("__roze_read_file("));
        assert!(java.contains("__roze_delete_file("));
        assert!(java.contains("__roze_read_lines("));
        // Every helper method must actually be defined in the class.
        assert!(java.contains("private static void __roze_write_file"));
        assert!(java.contains("private static void __roze_append_file"));
        assert!(java.contains("private static String __roze_read_file"));
        assert!(java.contains("private static boolean __roze_delete_file"));
        assert!(java.contains("private static java.util.List __roze_read_lines"));
    }

    #[test]
    fn file_exists_does_not_need_a_helper() {
        // Files.exists doesn't throw a checked exception, so it can be
        // called directly without a try/catch-wrapping helper.
        let java = generate("func main() { file_exists(\"a\"); }");
        assert!(java.contains("java.nio.file.Files.exists("));
        assert!(!java.contains("__roze_file_exists"));
    }

    // ---- IO: network ----

    #[test]
    fn network_intrinsics_delegate_to_helper_methods() {
        let java = generate("func main() { http_get(\"http://x\"); http_post(\"http://x\", \"body\"); }");
        assert!(java.contains("__roze_http_get("));
        assert!(java.contains("__roze_http_post("));
        assert!(java.contains("private static String __roze_http_get"));
        assert!(java.contains("private static String __roze_http_post"));
        assert!(java.contains("java.net.http.HttpClient"));
    }

    // ---- String escaping ----

    #[test]
    fn string_escapes_are_re_escaped_for_java() {
        // The Roze lexer resolves "\n" in source into a real newline
        // byte; codegen must re-escape it back to "\n" in the generated
        // Java text, or the .java file contains a raw newline inside a
        // string literal (invalid Java: "unclosed string literal").
        let java = generate("func main() { println(\"a\\nb\\tc\"); }");
        assert!(java.contains("\"a\\nb\\tc\""), "expected re-escaped \\n and \\t, got:\n{}", java);
        assert!(!java.contains("\"a\nb"), "must not contain a raw newline inside the Java string literal:\n{}", java);
    }

    #[test]
    fn backslash_and_quote_are_still_escaped() {
        let java = generate(r#"func main() { println("a\\b\"c"); }"#);
        assert!(java.contains(r#""a\\b\"c""#), "expected escaped backslash and quote, got:\n{}", java);
    }

    // ---- Web: JSON ----

    #[test]
    fn json_intrinsics_delegate_to_helper_methods() {
        let java = generate("func main() { let m = map_new(); json_encode(m); json_decode(\"{}\"); }");
        assert!(java.contains("__roze_json_encode("));
        assert!(java.contains("__roze_json_decode("));
        assert!(java.contains("private static String __roze_json_encode"));
        assert!(java.contains("private static Object __roze_json_decode"));
    }

    #[test]
    fn json_decode_result_is_castable_for_list_and_map_ops() {
        // json_decode's result is statically Object (its real shape
        // depends on the JSON text) -- list_get/map_get etc. must cast
        // rather than assume it's already java.util.List/Map.
        let java = generate("func main() { let d = json_decode(\"[]\"); list_length(d); }");
        assert!(java.contains("(java.util.List)("), "expected an explicit List cast for a json_decode result:\n{}", java);
    }

    // ---- Web: HTTP server ----

    #[test]
    fn http_server_intrinsics_delegate_to_helper_methods() {
        let java = generate(
            "func main() { let s = http_server_start(8080); let r = http_server_accept(s); http_server_respond(r, 200, \"ok\"); http_server_stop(s); }"
        );
        assert!(java.contains("__roze_http_server_start("));
        assert!(java.contains("__roze_http_server_accept("));
        assert!(java.contains("__roze_http_server_respond("));
        assert!(java.contains("__roze_http_server_stop("));
        assert!(java.contains("private static java.net.ServerSocket __roze_http_server_start"));
    }

    #[test]
    fn http_server_accept_returns_a_plain_map() {
        // The request is deliberately just a `map`, so it composes with
        // the existing map_get/map_has intrinsics with no new type.
        let java = generate("func main() { let s = http_server_start(8080); let r = http_server_accept(s); println(map_get(r, \"path\")); }");
        assert!(java.contains("java.util.Map r ="), "expected the request to be declared as java.util.Map:\n{}", java);
    }

    // ---- Database (SQL) ----

    #[test]
    fn sql_intrinsics_delegate_to_helper_methods_with_connection_cast() {
        // sql_connect returns a statically-Object-typed value (no
        // dedicated Connection type exists), so every other sql_*
        // intrinsic must cast it to java.sql.Connection at the call
        // site, or javac rejects passing an Object where Connection is
        // declared.
        let java = generate(
            "func main() { let c = sql_connect(\"jdbc:x\"); sql_execute(c, \"a\"); sql_query(c, \"b\"); sql_close(c); }"
        );
        assert!(java.contains("__roze_sql_connect("));
        assert!(java.contains("(java.sql.Connection)("), "expected an explicit Connection cast:\n{}", java);
        assert!(java.contains("private static java.sql.Connection __roze_sql_connect"));
        assert!(java.contains("private static java.util.List __roze_sql_query"));
        assert!(java.contains("private static int __roze_sql_execute"));
    }

    #[test]
    fn sql_query_returns_a_list_of_maps_shape() {
        let java = generate("func main() { let c = sql_connect(\"jdbc:x\"); let rows = sql_query(c, \"SELECT 1\"); }");
        assert!(java.contains("java.util.List rows ="), "expected sql_query's result to be typed as java.util.List:\n{}", java);
    }
}
