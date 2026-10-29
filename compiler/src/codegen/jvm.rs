use crate::ir::{TypedExpression, TypedExpressionKind, TypedPattern, TypedProgram, TypedStatement};
use crate::parser::ast::{BinaryOperator, UnaryOperator};
use crate::semantic::Type;
use anyhow::Result;

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

    private static java.util.Map __roze_http_request(String method, String url, java.util.Map headers, String body) {
        try {
            java.net.http.HttpClient client = java.net.http.HttpClient.newHttpClient();
            java.net.http.HttpRequest.Builder builder = java.net.http.HttpRequest.newBuilder()
                .uri(java.net.URI.create(url));
            if (headers != null) {
                for (Object k : headers.keySet()) {
                    Object v = headers.get(k);
                    if (k != null && v != null) {
                        builder.header(k.toString(), v.toString());
                    }
                }
            }
            java.net.http.HttpRequest.BodyPublisher pub = (body != null && !body.isEmpty())
                ? java.net.http.HttpRequest.BodyPublishers.ofString(body)
                : java.net.http.HttpRequest.BodyPublishers.noBody();
            builder.method(method.toUpperCase(), pub);
            java.net.http.HttpResponse<String> response = client.send(builder.build(), java.net.http.HttpResponse.BodyHandlers.ofString());
            java.util.Map<Object, Object> resMap = new java.util.HashMap<Object, Object>();
            resMap.put("status", (long) response.statusCode());
            resMap.put("body", response.body());
            java.util.Map<Object, Object> hMap = new java.util.HashMap<Object, Object>();
            response.headers().map().forEach((k, v) -> {
                if (!v.isEmpty()) hMap.put(k.toLowerCase(), v.get(0));
            });
            resMap.put("headers", hMap);
            return resMap;
        } catch (Exception e) {
            throw new RuntimeException("http_request failed: " + e.getMessage(), e);
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

    private static int __roze_to_int(Object v) {
        if (v == null) return 0;
        if (v instanceof Number) return ((Number) v).intValue();
        return Integer.parseInt(String.valueOf(v).trim());
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
    public static class RozeHttpServer {
        private final java.net.ServerSocket serverSocket;
        private final java.util.concurrent.ExecutorService executor;
        private final Thread acceptThread;
        private volatile boolean running = true;

        public RozeHttpServer(int port, RozeFn handler) throws java.io.IOException {
            this.serverSocket = new java.net.ServerSocket(port);
            java.util.concurrent.ExecutorService exec = null;
            try {
                // Java 21+ virtual thread per task executor via reflection
                exec = (java.util.concurrent.ExecutorService) java.util.concurrent.Executors.class
                    .getMethod("newVirtualThreadPerTaskExecutor").invoke(null);
            } catch (Throwable t) {
                // Fallback: cached thread pool (scales to thousands of concurrent workers)
                exec = java.util.concurrent.Executors.newCachedThreadPool();
            }
            this.executor = exec;

            this.acceptThread = new Thread(() -> {
                while (running && !serverSocket.isClosed()) {
                    try {
                        java.net.Socket socket = serverSocket.accept();
                        executor.submit(() -> {
                            try {
                                handleConnection(socket, handler);
                            } catch (Throwable t) {
                                try { socket.close(); } catch (Exception ignored) {}
                            }
                        });
                    } catch (java.io.IOException e) {
                        if (!running) break;
                    }
                }
            }, "Roze-HttpServer-Acceptor");
            this.acceptThread.setDaemon(true);
            this.acceptThread.start();
        }

        private static void handleConnection(java.net.Socket socket, RozeFn handler) {
            try {
                socket.setSoTimeout(30000);
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
                if (headerEnd < 0) {
                    socket.close();
                    return;
                }

                java.util.Map<Object, Object> request = __roze_parse_request_from_stream(
                    buffer.toByteArray(), headerEnd, socket, in, chunk
                );

                handler.apply(request);

                Boolean handled = (Boolean) request.get("__handled");
                if (handled == null || !handled) {
                    if (!socket.isClosed()) {
                        __roze_http_server_respond(request, 200, "OK");
                    }
                }
            } catch (Throwable t) {
                try {
                    if (!socket.isClosed()) {
                        String errMsg = t.getMessage() != null ? t.getMessage() : t.toString();
                        String errJson = "{\"error\":\"" + errMsg.replace("\"", "\\\"").replace("\n", " ") + "\"}";
                        byte[] b = errJson.getBytes("UTF-8");
                        String resp = "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: "
                            + b.length + "\r\nConnection: close\r\n\r\n" + errJson;
                        socket.getOutputStream().write(resp.getBytes("UTF-8"));
                        socket.getOutputStream().flush();
                        socket.close();
                    }
                } catch (Exception ignored) {}
            }
        }

        public void stop() {
            this.running = false;
            try {
                this.serverSocket.close();
            } catch (Exception ignored) {}
            this.executor.shutdownNow();
            try {
                this.acceptThread.join(1000);
            } catch (Exception ignored) {}
        }
    }

    private static java.net.ServerSocket __roze_http_server_start(int port) {
        try {
            return new java.net.ServerSocket(port);
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        }
    }

    private static RozeHttpServer __roze_http_server_listen(int port, Object handlerObj) {
        try {
            RozeFn handler = (RozeFn) handlerObj;
            return new RozeHttpServer(port, handler);
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

    private static java.util.Map<Object, Object> __roze_parse_request_from_stream(
        byte[] all, int headerEnd, java.net.Socket socket, java.io.InputStream in, byte[] chunk
    ) throws java.io.IOException {
        String headerText = new String(all, 0, Math.max(headerEnd, 0), "UTF-8");
        String[] lines = headerText.split("\r\n");
        String[] requestLineParts = lines.length > 0 ? lines[0].split(" ") : new String[0];
        String method = requestLineParts.length > 0 ? requestLineParts[0].toUpperCase() : "GET";
        String rawPath = requestLineParts.length > 1 ? requestLineParts[1] : "/";

        java.util.Map<Object, Object> headersMap = new java.util.HashMap<Object, Object>();
        int contentLength = 0;
        for (int i = 1; i < lines.length; i++) {
            String line = lines[i];
            int colon = line.indexOf(':');
            if (colon > 0) {
                String hKey = line.substring(0, colon).trim().toLowerCase();
                String hVal = line.substring(colon + 1).trim();
                headersMap.put(hKey, hVal);
                if (hKey.equals("content-length")) {
                    try { contentLength = Integer.parseInt(hVal); } catch (Exception ignored) {}
                }
            }
        }

        String path = rawPath;
        java.util.Map<Object, Object> queryMap = new java.util.HashMap<Object, Object>();
        int qIdx = rawPath.indexOf('?');
        if (qIdx >= 0) {
            path = rawPath.substring(0, qIdx);
            String queryString = rawPath.substring(qIdx + 1);
            String[] pairs = queryString.split("&");
            for (String pair : pairs) {
                if (!pair.isEmpty()) {
                    int eqIdx = pair.indexOf('=');
                    if (eqIdx >= 0) {
                        String k = java.net.URLDecoder.decode(pair.substring(0, eqIdx), "UTF-8");
                        String v = java.net.URLDecoder.decode(pair.substring(eqIdx + 1), "UTF-8");
                        queryMap.put(k, v);
                    } else {
                        queryMap.put(java.net.URLDecoder.decode(pair, "UTF-8"), "");
                    }
                }
            }
        }

        int bodyStart = headerEnd + 4;
        java.io.ByteArrayOutputStream bodyBuffer = new java.io.ByteArrayOutputStream();
        if (bodyStart < all.length) {
            bodyBuffer.write(all, bodyStart, all.length - bodyStart);
        }
        while (bodyBuffer.size() < contentLength) {
            int toRead = Math.min(chunk.length, contentLength - bodyBuffer.size());
            int n = in.read(chunk, 0, toRead);
            if (n <= 0) break;
            bodyBuffer.write(chunk, 0, n);
        }
        String body = bodyBuffer.toString("UTF-8");

        Object jsonObj = null;
        String trimmedBody = body.trim();
        if (!trimmedBody.isEmpty()) {
            String contentType = (String) headersMap.get("content-type");
            boolean isJson = (contentType != null && contentType.toLowerCase().contains("application/json"))
                    || (trimmedBody.startsWith("{") && trimmedBody.endsWith("}"))
                    || (trimmedBody.startsWith("[") && trimmedBody.endsWith("]"));
            if (isJson) {
                try {
                    jsonObj = __roze_json_decode(trimmedBody);
                } catch (Exception ignored) {}
            }
        }

        java.util.Map<Object, Object> request = new java.util.HashMap<Object, Object>();
        request.put("method", method);
        request.put("path", path);
        request.put("raw_path", rawPath);
        request.put("query", queryMap);
        request.put("headers", headersMap);
        request.put("body", body);
        request.put("json", jsonObj);
        request.put("params", new java.util.HashMap<Object, Object>());
        request.put("__socket", socket);
        request.put("__handled", false);
        return request;
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
            return __roze_parse_request_from_stream(all, headerEnd, socket, in, chunk);
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

    private static void __roze_http_server_respond(Object requestObj, int status, Object bodyObj) {
        try {
            java.util.Map request = (java.util.Map) requestObj;
            java.net.Socket socket = (java.net.Socket) request.get("__socket");
            String body = bodyObj != null ? String.valueOf(bodyObj) : "";
            byte[] bodyBytes = body.getBytes("UTF-8");
            String header = "HTTP/1.1 " + status + " " + __roze_http_status_text(status) + "\r\n"
                + "Content-Type: text/plain; charset=utf-8\r\n"
                + "Content-Length: " + bodyBytes.length + "\r\n"
                + "Connection: close\r\n\r\n";
            java.io.OutputStream out = socket.getOutputStream();
            out.write(header.getBytes("UTF-8"));
            if (bodyBytes.length > 0) {
                out.write(bodyBytes);
            }
            out.flush();
            socket.close();
            request.put("__handled", true);
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        }
    }

    private static void __roze_http_server_respond_json(Object requestObj, int status, Object data) {
        try {
            java.util.Map request = (java.util.Map) requestObj;
            java.net.Socket socket = (java.net.Socket) request.get("__socket");
            String jsonStr = __roze_json_encode(data);
            byte[] bodyBytes = jsonStr.getBytes("UTF-8");
            String header = "HTTP/1.1 " + status + " " + __roze_http_status_text(status) + "\r\n"
                + "Content-Type: application/json; charset=utf-8\r\n"
                + "Content-Length: " + bodyBytes.length + "\r\n"
                + "Connection: close\r\n\r\n";
            java.io.OutputStream out = socket.getOutputStream();
            out.write(header.getBytes("UTF-8"));
            out.write(bodyBytes);
            out.flush();
            socket.close();
            request.put("__handled", true);
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        }
    }

    private static void __roze_http_server_respond_headers(Object requestObj, int status, Object customHeadersObj, Object bodyObj) {
        try {
            java.util.Map request = (java.util.Map) requestObj;
            java.net.Socket socket = (java.net.Socket) request.get("__socket");
            java.util.Map customHeaders = customHeadersObj instanceof java.util.Map ? (java.util.Map) customHeadersObj : null;
            String body = bodyObj != null ? String.valueOf(bodyObj) : "";
            byte[] bodyBytes = body.getBytes("UTF-8");
            StringBuilder sb = new StringBuilder();
            sb.append("HTTP/1.1 ").append(status).append(" ").append(__roze_http_status_text(status)).append("\r\n");
            boolean hasContentType = false;
            if (customHeaders != null) {
                for (Object k : customHeaders.keySet()) {
                    String key = String.valueOf(k);
                    String val = String.valueOf(customHeaders.get(k));
                    if (key.equalsIgnoreCase("content-type")) hasContentType = true;
                    sb.append(key).append(": ").append(val).append("\r\n");
                }
            }
            if (!hasContentType) {
                sb.append("Content-Type: text/plain; charset=utf-8\r\n");
            }
            sb.append("Content-Length: ").append(bodyBytes.length).append("\r\n");
            sb.append("Connection: close\r\n\r\n");

            java.io.OutputStream out = socket.getOutputStream();
            out.write(sb.toString().getBytes("UTF-8"));
            if (bodyBytes.length > 0) {
                out.write(bodyBytes);
            }
            out.flush();
            socket.close();
            request.put("__handled", true);
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        }
    }

    private static void __roze_http_server_stop(Object serverObj) {
        if (serverObj instanceof RozeHttpServer) {
            ((RozeHttpServer) serverObj).stop();
        } else if (serverObj instanceof java.net.ServerSocket) {
            try {
                ((java.net.ServerSocket) serverObj).close();
            } catch (java.io.IOException e) {
                throw new RuntimeException(e);
            }
        }
    }

    private static java.util.Map __roze_url_route_match(Object patternObj, Object pathObj) {
        if (patternObj == null || pathObj == null) return null;
        String pattern = String.valueOf(patternObj);
        String path = String.valueOf(pathObj);
        int qIdx = path.indexOf('?');
        if (qIdx >= 0) {
            path = path.substring(0, qIdx);
        }
        String p1 = pattern.startsWith("/") ? pattern.substring(1) : pattern;
        String p2 = path.startsWith("/") ? path.substring(1) : path;
        if (p1.endsWith("/")) p1 = p1.substring(0, p1.length() - 1);
        if (p2.endsWith("/")) p2 = p2.substring(0, p2.length() - 1);

        String[] segs1 = p1.isEmpty() ? new String[0] : p1.split("/");
        String[] segs2 = p2.isEmpty() ? new String[0] : p2.split("/");

        java.util.Map<Object, Object> params = new java.util.HashMap<Object, Object>();

        int i = 0;
        int j = 0;
        while (i < segs1.length && j < segs2.length) {
            String s1 = segs1[i];
            String s2 = segs2[j];

            if (s1.startsWith("*")) {
                String paramName = s1.length() > 1 ? s1.substring(1) : "wildcard";
                StringBuilder rest = new StringBuilder();
                for (int k = j; k < segs2.length; k++) {
                    if (rest.length() > 0) rest.append("/");
                    rest.append(segs2[k]);
                }
                params.put(paramName, rest.toString());
                return params;
            } else if (s1.startsWith(":")) {
                String paramName = s1.substring(1);
                params.put(paramName, s2);
                i++;
                j++;
            } else if (s1.equals(s2)) {
                i++;
                j++;
            } else {
                return null;
            }
        }

        if (i == segs1.length - 1 && segs1[i].startsWith("*")) {
            String paramName = segs1[i].length() > 1 ? segs1[i].substring(1) : "wildcard";
            params.put(paramName, "");
            return params;
        }

        if (i == segs1.length && j == segs2.length) {
            return params;
        }
        return null;
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
                    String label = meta.getColumnLabel(i);
                    Object val = rs.getObject(i);
                    row.put(label, val);
                    row.put(label.toLowerCase(), val);
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

const ROZEBOOT_HELPER_METHODS: &str = r#"
    // ==============================================================================
    // RozeBoot Enterprise Helpers: Connection Pooling & JWT Security (ADV-03)
    // ==============================================================================

    public static class RozeConnectionPool {
        private final String url;
        private final int maxPoolSize;
        private final java.util.concurrent.BlockingQueue<java.sql.Connection> available;
        private final java.util.Set<java.sql.Connection> inUse;
        private final java.util.concurrent.atomic.AtomicInteger createdCount;
        private volatile boolean closed = false;
        private Object hikariDs = null;

        public RozeConnectionPool(String url, int maxPoolSize) {
            this.url = url;
            this.maxPoolSize = maxPoolSize > 0 ? maxPoolSize : 10;
            this.available = new java.util.concurrent.LinkedBlockingQueue<java.sql.Connection>();
            this.inUse = java.util.Collections.newSetFromMap(new java.util.concurrent.ConcurrentHashMap<java.sql.Connection, Boolean>());
            this.createdCount = new java.util.concurrent.atomic.AtomicInteger(0);

            try {
                if (url.startsWith("jdbc:sqlite:")) Class.forName("org.sqlite.JDBC");
                else if (url.startsWith("jdbc:h2:")) Class.forName("org.h2.Driver");
                else if (url.startsWith("jdbc:postgresql:")) Class.forName("org.postgresql.Driver");
                else if (url.startsWith("jdbc:mysql:")) Class.forName("com.mysql.cj.jdbc.Driver");
            } catch (Throwable ignored) {}

            // Dynamically detect and initialize HikariCP if present on classpath
            try {
                Class<?> cfgCls = Class.forName("com.zaxxer.hikari.HikariConfig");
                Class<?> dsCls = Class.forName("com.zaxxer.hikari.HikariDataSource");
                Object cfg = cfgCls.getConstructor().newInstance();
                cfgCls.getMethod("setJdbcUrl", String.class).invoke(cfg, url);
                cfgCls.getMethod("setMaximumPoolSize", int.class).invoke(cfg, this.maxPoolSize);
                this.hikariDs = dsCls.getConstructor(cfgCls).newInstance();
            } catch (Throwable t) {
                this.hikariDs = null;
            }
        }

        public java.sql.Connection getConnection() {
            if (closed) throw new RuntimeException("RozeConnectionPool is closed");
            if (hikariDs != null) {
                try {
                    return (java.sql.Connection) hikariDs.getClass().getMethod("getConnection").invoke(hikariDs);
                } catch (Exception e) {
                    throw new RuntimeException("HikariCP getConnection failed: " + e.getMessage(), e);
                }
            }

            java.sql.Connection conn = available.poll();
            if (conn != null) {
                try {
                    if (!conn.isClosed()) {
                        inUse.add(conn);
                        return conn;
                    }
                } catch (Exception ignored) {}
                try { conn.close(); } catch (Exception ignored) {}
                createdCount.decrementAndGet();
            }

            if (createdCount.get() < maxPoolSize) {
                try {
                    conn = java.sql.DriverManager.getConnection(url);
                    createdCount.incrementAndGet();
                    inUse.add(conn);
                    return conn;
                } catch (java.sql.SQLException e) {
                    throw new RuntimeException("Failed to open pooled connection: " + e.getMessage(), e);
                }
            }

            try {
                conn = available.poll(5, java.util.concurrent.TimeUnit.SECONDS);
                if (conn == null) throw new RuntimeException("Connection pool exhausted (timed out after 5s)");
                inUse.add(conn);
                return conn;
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                throw new RuntimeException("Interrupted waiting for pooled connection", e);
            }
        }

        public void releaseConnection(java.sql.Connection conn) {
            if (conn == null) return;
            if (hikariDs != null) {
                try { conn.close(); } catch (Exception ignored) {}
                return;
            }
            inUse.remove(conn);
            if (!closed) {
                try {
                    if (!conn.isClosed()) {
                        available.offer(conn);
                        return;
                    }
                } catch (Exception ignored) {}
            }
            try { conn.close(); } catch (Exception ignored) {}
            createdCount.decrementAndGet();
        }

        public void close() {
            closed = true;
            if (hikariDs != null) {
                try {
                    hikariDs.getClass().getMethod("close").invoke(hikariDs);
                } catch (Exception ignored) {}
                return;
            }
            java.sql.Connection conn;
            while ((conn = available.poll()) != null) {
                try { conn.close(); } catch (Exception ignored) {}
            }
            for (java.sql.Connection c : inUse) {
                try { c.close(); } catch (Exception ignored) {}
            }
            inUse.clear();
        }

        public java.util.Map<Object, Object> getStats() {
            java.util.Map<Object, Object> s = new java.util.HashMap<Object, Object>();
            s.put("max_pool_size", (long) maxPoolSize);
            s.put("active_connections", (long) inUse.size());
            s.put("active", (long) inUse.size());
            s.put("idle_connections", (long) available.size());
            s.put("idle", (long) available.size());
            s.put("total_connections", (long) createdCount.get());
            s.put("total", (long) createdCount.get());
            s.put("is_hikari", hikariDs != null);
            s.put("is_closed", closed);
            s.put("closed", closed);
            return s;
        }
    }

    private static Object __roze_db_pool_create(String url, long maxPoolSize) {
        return new RozeConnectionPool(url, (int) maxPoolSize);
    }

    private static Object __roze_db_pool_get(Object poolObj) {
        if (poolObj instanceof RozeConnectionPool) {
            return ((RozeConnectionPool) poolObj).getConnection();
        }
        throw new RuntimeException("Expected RozeConnectionPool");
    }

    private static void __roze_db_pool_release(Object poolObj, Object connObj) {
        if (poolObj instanceof RozeConnectionPool && connObj instanceof java.sql.Connection) {
            ((RozeConnectionPool) poolObj).releaseConnection((java.sql.Connection) connObj);
        }
    }

    private static void __roze_db_pool_close(Object poolObj) {
        if (poolObj instanceof RozeConnectionPool) {
            ((RozeConnectionPool) poolObj).close();
        }
    }

    private static java.util.Map __roze_db_pool_stats(Object poolObj) {
        if (poolObj instanceof RozeConnectionPool) {
            return ((RozeConnectionPool) poolObj).getStats();
        }
        return new java.util.HashMap();
    }

    private static java.util.List __roze_db_pool_query(Object poolObj, String sql) {
        if (!(poolObj instanceof RozeConnectionPool)) throw new RuntimeException("Expected RozeConnectionPool");
        RozeConnectionPool pool = (RozeConnectionPool) poolObj;
        java.sql.Connection conn = pool.getConnection();
        try {
            return __roze_sql_query(conn, sql);
        } finally {
            pool.releaseConnection(conn);
        }
    }

    private static int __roze_db_pool_execute(Object poolObj, String sql) {
        if (!(poolObj instanceof RozeConnectionPool)) throw new RuntimeException("Expected RozeConnectionPool");
        RozeConnectionPool pool = (RozeConnectionPool) poolObj;
        java.sql.Connection conn = pool.getConnection();
        try {
            return __roze_sql_execute(conn, sql);
        } finally {
            pool.releaseConnection(conn);
        }
    }

    // --- JWT Security ---
    private static String __roze_base64_url_encode(byte[] data) {
        return java.util.Base64.getUrlEncoder().withoutPadding().encodeToString(data);
    }

    private static byte[] __roze_base64_url_decode(String str) {
        return java.util.Base64.getUrlDecoder().decode(str);
    }

    private static String __roze_hmac_sha256(String data, String secret) {
        try {
            javax.crypto.Mac mac = javax.crypto.Mac.getInstance("HmacSHA256");
            javax.crypto.spec.SecretKeySpec k = new javax.crypto.spec.SecretKeySpec(secret.getBytes(java.nio.charset.StandardCharsets.UTF_8), "HmacSHA256");
            mac.init(k);
            byte[] h = mac.doFinal(data.getBytes(java.nio.charset.StandardCharsets.UTF_8));
            return __roze_base64_url_encode(h);
        } catch (Exception e) {
            throw new RuntimeException("HMAC-SHA256 failed: " + e.getMessage(), e);
        }
    }

    private static String __roze_jwt_sign(java.util.Map claims, String secret) {
        java.util.Map<String, String> header = new java.util.HashMap<String, String>();
        header.put("alg", "HS256");
        header.put("typ", "JWT");

        String hJson = __roze_json_encode(header);
        String pJson = __roze_json_encode(claims);

        String part0 = __roze_base64_url_encode(hJson.getBytes(java.nio.charset.StandardCharsets.UTF_8));
        String part1 = __roze_base64_url_encode(pJson.getBytes(java.nio.charset.StandardCharsets.UTF_8));
        String signingInput = part0 + "." + part1;
        String sig = __roze_hmac_sha256(signingInput, secret);
        return signingInput + "." + sig;
    }

    private static boolean __roze_jwt_verify(String token, String secret) {
        if (token == null || secret == null) return false;
        String[] parts = token.split("\\.");
        if (parts.length != 3) return false;

        String signingInput = parts[0] + "." + parts[1];
        String expectedSig = __roze_hmac_sha256(signingInput, secret);
        if (!java.security.MessageDigest.isEqual(expectedSig.getBytes(java.nio.charset.StandardCharsets.UTF_8), parts[2].getBytes(java.nio.charset.StandardCharsets.UTF_8))) {
            return false;
        }

        try {
            byte[] pBytes = __roze_base64_url_decode(parts[1]);
            String pJson = new String(pBytes, java.nio.charset.StandardCharsets.UTF_8);
            Object decoded = __roze_json_decode(pJson);
            if (decoded instanceof java.util.Map) {
                java.util.Map m = (java.util.Map) decoded;
                if (m.containsKey("exp")) {
                    Object expVal = m.get("exp");
                    long exp = 0;
                    if (expVal instanceof Number) {
                        exp = ((Number) expVal).longValue();
                    }
                    long now = System.currentTimeMillis() / 1000L;
                    if (exp > 0 && now >= exp) {
                        return false;
                    }
                }
            }
        } catch (Exception e) {
            return false;
        }

        return true;
    }

    private static java.util.Map __roze_jwt_decode(String token) {
        if (token == null) return new java.util.HashMap();
        String[] parts = token.split("\\.");
        if (parts.length != 3) return new java.util.HashMap();
        try {
            byte[] pBytes = __roze_base64_url_decode(parts[1]);
            String pJson = new String(pBytes, java.nio.charset.StandardCharsets.UTF_8);
            Object decoded = __roze_json_decode(pJson);
            if (decoded instanceof java.util.Map) {
                return (java.util.Map) decoded;
            }
        } catch (Exception ignored) {}
        return new java.util.HashMap();
    }

    private static java.util.Map __roze_jwt_decode_header(String token) {
        if (token == null) return new java.util.HashMap();
        String[] parts = token.split("\\.");
        if (parts.length != 3) return new java.util.HashMap();
        try {
            byte[] hBytes = __roze_base64_url_decode(parts[0]);
            String hJson = new String(hBytes, java.nio.charset.StandardCharsets.UTF_8);
            Object decoded = __roze_json_decode(hJson);
            if (decoded instanceof java.util.Map) {
                return (java.util.Map) decoded;
            }
        } catch (Exception ignored) {}
        return new java.util.HashMap();
    }

    // --- Time ---
    private static int __roze_time_now() {
        return (int) (System.currentTimeMillis() / 1000L);
    }

    private static int __roze_time_now_millis() {
        return (int) (System.currentTimeMillis() % 2000000000L);
    }
"#;


const OPTION_RESULT_HELPER_CLASSES: &str = r#"
class RozeOption {
    public final boolean is_some;
    public final Object value;
    public RozeOption(boolean is_some, Object value) {
        this.is_some = is_some;
        this.value = value;
    }
    public static RozeOption some(Object val) { return new RozeOption(true, val); }
    public static RozeOption none() { return new RozeOption(false, null); }
    public boolean isSome() { return is_some; }
    public boolean isNone() { return !is_some; }
    public Object unwrap() {
        if (!is_some) throw new RuntimeException("Called unwrap() on a None Option");
        return value;
    }
    public Object unwrapOr(Object defaultVal) {
        return is_some ? value : defaultVal;
    }
    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof RozeOption)) return false;
        RozeOption other = (RozeOption) o;
        if (this.is_some != other.is_some) return false;
        return java.util.Objects.equals(this.value, other.value);
    }
    @Override
    public int hashCode() {
        return java.util.Objects.hash(is_some, value);
    }
    @Override
    public String toString() {
        return is_some ? ("Some(" + value + ")") : "None";
    }
}

class RozeResult {
    public final boolean is_ok;
    public final Object value;
    public final Object error;
    public RozeResult(boolean is_ok, Object value, Object error) {
        this.is_ok = is_ok;
        this.value = value;
        this.error = error;
    }
    public static RozeResult ok(Object val) { return new RozeResult(true, val, null); }
    public static RozeResult err(Object err) { return new RozeResult(false, null, err); }
    public boolean isOk() { return is_ok; }
    public boolean isErr() { return !is_ok; }
    public Object unwrap() {
        if (!is_ok) throw new RuntimeException("Called unwrap() on an Err Result: " + error);
        return value;
    }
    public Object unwrapErr() {
        if (is_ok) throw new RuntimeException("Called unwrap_err() on an Ok Result: " + value);
        return error;
    }
    public Object unwrapOr(Object defaultVal) {
        return is_ok ? value : defaultVal;
    }
    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof RozeResult)) return false;
        RozeResult other = (RozeResult) o;
        if (this.is_ok != other.is_ok) return false;
        return this.is_ok ? java.util.Objects.equals(this.value, other.value)
                          : java.util.Objects.equals(this.error, other.error);
    }
    @Override
    public int hashCode() {
        return java.util.Objects.hash(is_ok, value, error);
    }
    @Override
    public String toString() {
        return is_ok ? ("Ok(" + value + ")") : ("Err(" + error + ")");
    }
}
"#;

const OPTION_RESULT_HELPER_METHODS: &str = r#"
    private static Object __roze_unwrap(Object o) {
        if (o instanceof RozeResult) return ((RozeResult) o).unwrap();
        if (o instanceof RozeOption) return ((RozeOption) o).unwrap();
        throw new RuntimeException("Cannot unwrap non-Result/Option value: " + o);
    }

    private static Object __roze_unwrap_or(Object o, Object defaultVal) {
        if (o instanceof RozeResult) return ((RozeResult) o).unwrapOr(defaultVal);
        if (o instanceof RozeOption) return ((RozeOption) o).unwrapOr(defaultVal);
        return defaultVal;
    }

    private static boolean __roze_is_err_or_none(Object o) {
        if (o instanceof RozeResult) return !((RozeResult) o).is_ok;
        if (o instanceof RozeOption) return !((RozeOption) o).is_some;
        return false;
    }

    private static Object __roze_unwrap_try(Object o) {
        if (o instanceof RozeResult) return ((RozeResult) o).unwrap();
        if (o instanceof RozeOption) return ((RozeOption) o).unwrap();
        return o;
    }

    private static RozeResult __roze_read_file_result(String path) {
        try {
            return RozeResult.ok(new String(java.nio.file.Files.readAllBytes(java.nio.file.Paths.get(path))));
        } catch (Exception e) {
            String msg = e.getMessage() != null ? e.getMessage() : e.toString();
            return RozeResult.err(msg);
        }
    }

    private static RozeResult __roze_write_file_result(String path, String content) {
        try {
            java.nio.file.Files.write(java.nio.file.Paths.get(path), content.getBytes());
            return RozeResult.ok(true);
        } catch (Exception e) {
            String msg = e.getMessage() != null ? e.getMessage() : e.toString();
            return RozeResult.err(msg);
        }
    }

    private static RozeResult __roze_append_file_result(String path, String content) {
        try {
            java.nio.file.Files.write(
                java.nio.file.Paths.get(path),
                content.getBytes(),
                java.nio.file.StandardOpenOption.CREATE,
                java.nio.file.StandardOpenOption.APPEND
            );
            return RozeResult.ok(true);
        } catch (Exception e) {
            String msg = e.getMessage() != null ? e.getMessage() : e.toString();
            return RozeResult.err(msg);
        }
    }

    private static RozeResult __roze_delete_file_result(String path) {
        try {
            boolean deleted = java.nio.file.Files.deleteIfExists(java.nio.file.Paths.get(path));
            return RozeResult.ok(deleted);
        } catch (Exception e) {
            String msg = e.getMessage() != null ? e.getMessage() : e.toString();
            return RozeResult.err(msg);
        }
    }

    private static RozeResult __roze_read_lines_result(String path) {
        try {
            return RozeResult.ok(new java.util.ArrayList(java.nio.file.Files.readAllLines(java.nio.file.Paths.get(path))));
        } catch (Exception e) {
            String msg = e.getMessage() != null ? e.getMessage() : e.toString();
            return RozeResult.err(msg);
        }
    }

    private static RozeResult __roze_http_get_result(String url) {
        try {
            java.net.http.HttpClient client = java.net.http.HttpClient.newHttpClient();
            java.net.http.HttpRequest request = java.net.http.HttpRequest.newBuilder()
                .uri(java.net.URI.create(url))
                .GET()
                .build();
            java.net.http.HttpResponse<String> response = client.send(request, java.net.http.HttpResponse.BodyHandlers.ofString());
            return RozeResult.ok(response.body());
        } catch (Exception e) {
            String msg = e.getMessage() != null ? e.getMessage() : e.toString();
            return RozeResult.err(msg);
        }
    }

    private static RozeResult __roze_http_post_result(String url, String body) {
        try {
            java.net.http.HttpClient client = java.net.http.HttpClient.newHttpClient();
            java.net.http.HttpRequest request = java.net.http.HttpRequest.newBuilder()
                .uri(java.net.URI.create(url))
                .header("Content-Type", "application/json")
                .POST(java.net.http.HttpRequest.BodyPublishers.ofString(body))
                .build();
            java.net.http.HttpResponse<String> response = client.send(request, java.net.http.HttpResponse.BodyHandlers.ofString());
            return RozeResult.ok(response.body());
        } catch (Exception e) {
            String msg = e.getMessage() != null ? e.getMessage() : e.toString();
            return RozeResult.err(msg);
        }
    }

    private static RozeResult __roze_to_int_result(String s) {
        try {
            return RozeResult.ok(Integer.parseInt(s.trim()));
        } catch (Exception e) {
            return RozeResult.err("Cannot parse integer from '" + s + "'");
        }
    }

    private static RozeOption __roze_list_get_option(java.util.List list, int idx) {
        if (list != null && idx >= 0 && idx < list.size()) {
            return RozeOption.some(list.get(idx));
        }
        return RozeOption.none();
    }

    private static RozeOption __roze_map_get_option(java.util.Map map, Object key) {
        if (map != null && map.containsKey(key)) {
            return RozeOption.some(map.get(key));
        }
        return RozeOption.none();
    }
"#;

const CONCURRENCY_HELPER_CLASSES: &str = r#"
class RozeFiberHandle {
    public final Thread thread;
    public volatile Object result = null;
    public volatile Throwable error = null;

    public RozeFiberHandle(java.util.concurrent.Callable<Object> task) {
        Runnable runnable = () -> {
            try {
                this.result = task.call();
            } catch (Throwable t) {
                this.error = t;
            }
        };
        Thread t = null;
        try {
            t = Thread.ofVirtual().name("roze-fiber-", 0).start(runnable);
        } catch (Throwable fallback) {
            t = new Thread(runnable);
            t.start();
        }
        this.thread = t;
    }

    public Object join() {
        try {
            this.thread.join();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new RuntimeException(e);
        }
        if (this.error != null) {
            if (this.error instanceof RuntimeException) throw (RuntimeException) this.error;
            throw new RuntimeException(this.error);
        }
        return this.result;
    }
}

class RozeThreadHandle {
    public final Thread thread;
    public volatile Object result = null;
    public volatile Throwable error = null;

    public RozeThreadHandle(java.util.concurrent.Callable<Object> task) {
        Runnable runnable = () -> {
            try {
                this.result = task.call();
            } catch (Throwable t) {
                this.error = t;
            }
        };
        Thread t = null;
        try {
            Object ofVirtual = Thread.class.getMethod("ofVirtual").invoke(null);
            t = (Thread) ofVirtual.getClass().getMethod("start", Runnable.class).invoke(ofVirtual, runnable);
        } catch (Throwable ignored) {
            t = new Thread(runnable);
            t.start();
        }
        this.thread = t;
    }

    public Object join() {
        try {
            this.thread.join();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new RuntimeException(e);
        }
        if (this.error != null) {
            if (this.error instanceof RuntimeException) throw (RuntimeException) this.error;
            throw new RuntimeException(this.error);
        }
        return this.result;
    }
}

class RozeChannel {
    private Object[] buffer;
    private final int capacity;
    private int allocCap;
    private int head = 0;
    private int tail = 0;
    private int count = 0;
    private boolean closed = false;
    private final java.util.concurrent.locks.ReentrantLock lock = new java.util.concurrent.locks.ReentrantLock();
    private final java.util.concurrent.locks.Condition notEmpty = lock.newCondition();
    private final java.util.concurrent.locks.Condition notFull = lock.newCondition();

    public RozeChannel(int capacity) {
        if (capacity <= 0 && capacity != -1) capacity = 64;
        this.capacity = capacity;
        this.allocCap = (capacity > 0) ? capacity : 64;
        this.buffer = new Object[this.allocCap];
    }

    private void ensureCapacity(int minCap) {
        if (minCap > allocCap) {
            int newCap = allocCap * 2;
            Object[] newBuf = new Object[newCap];
            for (int i = 0; i < count; i++) {
                newBuf[i] = buffer[(head + i) % allocCap];
            }
            buffer = newBuf;
            head = 0;
            tail = count;
            allocCap = newCap;
        }
    }

    public boolean send(Object val) {
        lock.lock();
        try {
            if (capacity > 0) {
                while (count == capacity && !closed) {
                    notFull.await();
                }
            }
            if (closed) return false;
            ensureCapacity(count + 1);
            buffer[tail] = val;
            tail = (tail + 1) % allocCap;
            count++;
            notEmpty.signal();
            return true;
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return false;
        } finally {
            lock.unlock();
        }
    }

    public boolean trySend(Object val) {
        if (!lock.tryLock()) return false;
        try {
            if (closed || (capacity > 0 && count >= capacity)) return false;
            ensureCapacity(count + 1);
            buffer[tail] = val;
            tail = (tail + 1) % allocCap;
            count++;
            notEmpty.signal();
            return true;
        } finally {
            lock.unlock();
        }
    }

    public Object recv() {
        lock.lock();
        try {
            while (count == 0 && !closed) {
                notEmpty.await();
            }
            if (count == 0 && closed) return null;
            Object val = buffer[head];
            buffer[head] = null;
            head = (head + 1) % allocCap;
            count--;
            notFull.signal();
            return val;
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return null;
        } finally {
            lock.unlock();
        }
    }

    public Object tryRecv() {
        if (!lock.tryLock()) return null;
        try {
            if (count == 0) return null;
            Object val = buffer[head];
            buffer[head] = null;
            head = (head + 1) % allocCap;
            count--;
            notFull.signal();
            return val;
        } finally {
            lock.unlock();
        }
    }

    public void close() {
        lock.lock();
        try {
            closed = true;
            notEmpty.signalAll();
            notFull.signalAll();
        } finally {
            lock.unlock();
        }
    }

    public boolean isClosed() {
        lock.lock();
        try {
            return closed;
        } finally {
            lock.unlock();
        }
    }

    public int len() {
        lock.lock();
        try {
            return count;
        } finally {
            lock.unlock();
        }
    }

    public int getCapacity() {
        return capacity;
    }
}

class RozeMutex {
    public final java.util.concurrent.locks.ReentrantLock lock = new java.util.concurrent.locks.ReentrantLock();

    public void lock() {
        lock.lock();
    }

    public void unlock() {
        if (lock.isHeldByCurrentThread()) {
            lock.unlock();
        }
    }

    public boolean tryLock() {
        return lock.tryLock();
    }
}

class RozeRwLock {
    public final java.util.concurrent.locks.ReentrantReadWriteLock rwlock = new java.util.concurrent.locks.ReentrantReadWriteLock();

    public void readLock() {
        rwlock.readLock().lock();
    }

    public void readUnlock() {
        rwlock.readLock().unlock();
    }

    public void writeLock() {
        rwlock.writeLock().lock();
    }

    public void writeUnlock() {
        if (rwlock.isWriteLockedByCurrentThread()) {
            rwlock.writeLock().unlock();
        }
    }

    public boolean tryReadLock() {
        return rwlock.readLock().tryLock();
    }

    public boolean tryWriteLock() {
        return rwlock.writeLock().tryLock();
    }
}

class RozeFuture {
    public final java.util.concurrent.CompletableFuture<Object> cf;

    public RozeFuture(java.util.concurrent.CompletableFuture<Object> cf) {
        this.cf = cf;
    }

    public static RozeFuture supply(RozeFn fn) {
        java.util.concurrent.CompletableFuture<Object> cf = new java.util.concurrent.CompletableFuture<>();
        Runnable task = () -> {
            try {
                Object res = fn.apply();
                cf.complete(res);
            } catch (Throwable t) {
                cf.completeExceptionally(t);
            }
        };
        try {
            Object ofVirtual = Thread.class.getMethod("ofVirtual").invoke(null);
            ofVirtual.getClass().getMethod("start", Runnable.class).invoke(ofVirtual, task);
        } catch (Throwable fallback) {
            Thread t = new Thread(task);
            t.setDaemon(true);
            t.start();
        }
        return new RozeFuture(cf);
    }

    public static RozeFuture completed(Object val) {
        return new RozeFuture(java.util.concurrent.CompletableFuture.completedFuture(val));
    }

    public Object awaitResult() {
        try {
            return this.cf.join();
        } catch (java.util.concurrent.CompletionException ce) {
            Throwable cause = ce.getCause();
            if (cause instanceof RuntimeException) throw (RuntimeException) cause;
            throw new RuntimeException(cause);
        }
    }

    public boolean isDone() {
        return this.cf.isDone();
    }
}

class RozeCancellationToken {
    public final java.util.concurrent.atomic.AtomicBoolean cancelled = new java.util.concurrent.atomic.AtomicBoolean(false);

    public void cancel() {
        cancelled.set(true);
    }

    public boolean isCancelled() {
        return cancelled.get();
    }
}

class RozeSubtask {
    public final java.util.concurrent.CompletableFuture<Object> future = new java.util.concurrent.CompletableFuture<>();
    public volatile Object result = null;
    public volatile boolean isDone = false;
    public volatile boolean isCancelled = false;

    public Object get() {
        try {
            return future.join();
        } catch (java.util.concurrent.CompletionException ce) {
            Throwable cause = ce.getCause();
            if (cause instanceof RuntimeException) throw (RuntimeException) cause;
            throw new RuntimeException(cause);
        }
    }

    public boolean isDone() {
        return isDone;
    }

    public boolean isCancelled() {
        return isCancelled;
    }
}

class RozeTaskGroup {
    public final int policy; // 0 = ALL, 1 = SHUTDOWN_ON_FAILURE, 2 = SHUTDOWN_ON_SUCCESS
    public final RozeCancellationToken token = new RozeCancellationToken();
    public final java.util.List<RozeSubtask> tasks = new java.util.concurrent.CopyOnWriteArrayList<>();
    public final java.util.concurrent.atomic.AtomicInteger completedCount = new java.util.concurrent.atomic.AtomicInteger(0);
    public final java.util.concurrent.atomic.AtomicInteger failedCount = new java.util.concurrent.atomic.AtomicInteger(0);
    public final java.util.concurrent.atomic.AtomicBoolean hasSuccessResult = new java.util.concurrent.atomic.AtomicBoolean(false);
    public volatile Object successResult = null;
    public final java.util.concurrent.locks.ReentrantLock lock = new java.util.concurrent.locks.ReentrantLock();
    public final java.util.concurrent.locks.Condition cond = lock.newCondition();
    public volatile boolean joined = false;
    private final Thread timeoutThread;

    public RozeTaskGroup(int policy, long timeoutMs) {
        this.policy = policy;
        if (timeoutMs > 0) {
            Runnable watcher = () -> {
                try {
                    Thread.sleep(timeoutMs);
                    if (!joined) {
                        cancel();
                    }
                } catch (InterruptedException ignored) {}
            };
            Thread t;
            try {
                Object ofVirtual = Thread.class.getMethod("ofVirtual").invoke(null);
                t = (Thread) ofVirtual.getClass().getMethod("start", Runnable.class).invoke(ofVirtual, watcher);
            } catch (Throwable fallback) {
                t = new Thread(watcher);
                t.setDaemon(true);
                t.start();
            }
            this.timeoutThread = t;
        } else {
            this.timeoutThread = null;
        }
    }

    public RozeSubtask fork(RozeFn fn) {
        RozeSubtask sub = new RozeSubtask();
        tasks.add(sub);

        Runnable runner = () -> {
            if (token.isCancelled()) {
                sub.isCancelled = true;
                sub.isDone = true;
                sub.future.complete(null);
                failedCount.incrementAndGet();
                completedCount.incrementAndGet();
                lock.lock();
                try {
                    cond.signalAll();
                } finally {
                    lock.unlock();
                }
                return;
            }

            try {
                Object res = fn.apply();
                if (token.isCancelled()) {
                    sub.isCancelled = true;
                    sub.isDone = true;
                    sub.future.complete(null);
                    failedCount.incrementAndGet();
                } else {
                    sub.result = res;
                    sub.isDone = true;
                    sub.future.complete(res);

                    if (policy == 2) { // SHUTDOWN_ON_SUCCESS
                        if (hasSuccessResult.compareAndSet(false, true)) {
                            successResult = res;
                            cancel();
                        }
                    }
                }
            } catch (Throwable t) {
                sub.isCancelled = true;
                sub.isDone = true;
                sub.future.completeExceptionally(t);
                failedCount.incrementAndGet();
                if (policy == 1) { // SHUTDOWN_ON_FAILURE
                    cancel();
                }
            } finally {
                completedCount.incrementAndGet();
                lock.lock();
                try {
                    cond.signalAll();
                } finally {
                    lock.unlock();
                }
            }
        };

        try {
            Object ofVirtual = Thread.class.getMethod("ofVirtual").invoke(null);
            ofVirtual.getClass().getMethod("start", Runnable.class).invoke(ofVirtual, runner);
        } catch (Throwable fallback) {
            Thread t = new Thread(runner);
            t.setDaemon(true);
            t.start();
        }
        return sub;
    }

    public int join() {
        lock.lock();
        try {
            while (true) {
                if (policy == 2 && hasSuccessResult.get()) break;
                if (policy == 1 && failedCount.get() > 0) break;
                if (completedCount.get() >= tasks.size()) break;
                try {
                    cond.await();
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    break;
                }
            }
            joined = true;
        } finally {
            lock.unlock();
        }

        if (hasSuccessResult.get() && successResult instanceof Number) {
            return ((Number) successResult).intValue();
        }
        return 0;
    }

    public void cancel() {
        token.cancel();
        lock.lock();
        try {
            cond.signalAll();
        } finally {
            lock.unlock();
        }
    }

    public boolean isCancelled() {
        return token.isCancelled();
    }

    public int result() {
        if (successResult instanceof Number) {
            return ((Number) successResult).intValue();
        }
        return 0;
    }

    public RozeCancellationToken token() {
        return token;
    }

    public int taskCount() {
        return tasks.size();
    }

    public int completedCount() {
        return completedCount.get();
    }
}
"#;

const CONCURRENCY_HELPER_METHODS: &str = r#"
    private static final java.util.concurrent.atomic.AtomicLong __ROZE_ACTIVE_FIBERS = new java.util.concurrent.atomic.AtomicLong(0);

    private static RozeFuture __roze_async_spawn(Object fnObj) {
        RozeFn fn = (RozeFn) fnObj;
        return RozeFuture.supply(fn);
    }

    private static Object __roze_future_await(Object futObj) {
        if (futObj instanceof RozeFuture) {
            return ((RozeFuture) futObj).awaitResult();
        }
        return futObj;
    }

    private static boolean __roze_future_poll(Object futObj) {
        if (futObj instanceof RozeFuture) {
            return ((RozeFuture) futObj).isDone();
        }
        return true;
    }

    private static RozeFuture __roze_future_ready(Object val) {
        return RozeFuture.completed(val);
    }

    private static RozeFuture __roze_async_sleep(int ms) {
        java.util.concurrent.CompletableFuture<Object> cf = new java.util.concurrent.CompletableFuture<>();
        Runnable sleeper = () -> {
            try {
                Thread.sleep(ms);
                cf.complete((long) ms);
            } catch (InterruptedException e) {
                cf.completeExceptionally(e);
            }
        };
        try {
            Object ofVirtual = Thread.class.getMethod("ofVirtual").invoke(null);
            ofVirtual.getClass().getMethod("start", Runnable.class).invoke(ofVirtual, sleeper);
        } catch (Throwable fallback) {
            Thread t = new Thread(sleeper);
            t.setDaemon(true);
            t.start();
        }
        return new RozeFuture(cf);
    }

    private static RozeFiberHandle __roze_fiber_spawn(Object fnObj) {
        RozeFn fn = (RozeFn) fnObj;
        __ROZE_ACTIVE_FIBERS.incrementAndGet();
        return new RozeFiberHandle(() -> {
            try {
                return fn.apply();
            } finally {
                __ROZE_ACTIVE_FIBERS.decrementAndGet();
            }
        });
    }

    private static Object __roze_fiber_join(Object handleObj) {
        if (handleObj instanceof RozeFiberHandle) {
            return ((RozeFiberHandle) handleObj).join();
        } else if (handleObj instanceof RozeThreadHandle) {
            return ((RozeThreadHandle) handleObj).join();
        }
        return null;
    }

    private static void __roze_fiber_yield() {
        Thread.yield();
    }

    private static void __roze_fiber_sleep(long ms) {
        if (ms <= 0) {
            Thread.yield();
            return;
        }
        try {
            Thread.sleep(ms);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }

    private static int __roze_fiber_id() {
        return (int) Thread.currentThread().threadId();
    }

    private static int __roze_fiber_count() {
        return (int) __ROZE_ACTIVE_FIBERS.get();
    }

    private static void __roze_fiber_stats() {
        int active = (int) __ROZE_ACTIVE_FIBERS.get();
        System.out.println("[Roze Project Loom Virtual Thread Scheduler] Active: " + active + " Carrier: ForkJoinPool");
    }

    private static RozeThreadHandle __roze_thread_spawn(Object fnObj) {
        RozeFn fn = (RozeFn) fnObj;
        return new RozeThreadHandle(() -> fn.apply());
    }

    private static Object __roze_thread_join(Object handleObj) {
        if (handleObj instanceof RozeThreadHandle) {
            return ((RozeThreadHandle) handleObj).join();
        } else if (handleObj instanceof RozeFiberHandle) {
            return ((RozeFiberHandle) handleObj).join();
        }
        return null;
    }

    private static void __roze_thread_sleep(long ms) {
        if (ms <= 0) return;
        try {
            Thread.sleep(ms);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }

    private static int __roze_thread_current_id() {
        return (int) Thread.currentThread().getId();
    }

    private static RozeChannel __roze_channel_new(int capacity) {
        return new RozeChannel(capacity);
    }

    private static boolean __roze_channel_send(Object chan, Object val) {
        if (chan instanceof RozeChannel) {
            return ((RozeChannel) chan).send(val);
        }
        return false;
    }

    private static Object __roze_channel_recv(Object chan) {
        if (chan instanceof RozeChannel) {
            return ((RozeChannel) chan).recv();
        }
        return null;
    }

    private static void __roze_channel_close(Object chan) {
        if (chan instanceof RozeChannel) {
            ((RozeChannel) chan).close();
        }
    }

    private static boolean __roze_channel_is_closed(Object chan) {
        if (chan instanceof RozeChannel) {
            return ((RozeChannel) chan).isClosed();
        }
        return true;
    }

    private static int __roze_channel_len(Object chan) {
        if (chan instanceof RozeChannel) {
            return ((RozeChannel) chan).len();
        }
        return 0;
    }

    private static RozeChannel __roze_channel_unbounded() {
        return new RozeChannel(-1);
    }

    private static boolean __roze_channel_try_send(Object chan, Object val) {
        if (chan instanceof RozeChannel) {
            return ((RozeChannel) chan).trySend(val);
        }
        return false;
    }

    private static Object __roze_channel_try_recv(Object chan) {
        if (chan instanceof RozeChannel) {
            return ((RozeChannel) chan).tryRecv();
        }
        return null;
    }

    private static int __roze_channel_capacity(Object chan) {
        if (chan instanceof RozeChannel) {
            return ((RozeChannel) chan).getCapacity();
        }
        return 0;
    }

    private static RozeMutex __roze_mutex_new() {
        return new RozeMutex();
    }

    private static void __roze_mutex_lock(Object m) {
        if (m instanceof RozeMutex) ((RozeMutex) m).lock();
    }

    private static void __roze_mutex_unlock(Object m) {
        if (m instanceof RozeMutex) ((RozeMutex) m).unlock();
    }

    private static boolean __roze_mutex_try_lock(Object m) {
        if (m instanceof RozeMutex) return ((RozeMutex) m).tryLock();
        return false;
    }

    private static RozeRwLock __roze_rwlock_new() {
        return new RozeRwLock();
    }

    private static void __roze_rwlock_read_lock(Object rw) {
        if (rw instanceof RozeRwLock) ((RozeRwLock) rw).readLock();
    }

    private static void __roze_rwlock_read_unlock(Object rw) {
        if (rw instanceof RozeRwLock) ((RozeRwLock) rw).readUnlock();
    }

    private static void __roze_rwlock_write_lock(Object rw) {
        if (rw instanceof RozeRwLock) ((RozeRwLock) rw).writeLock();
    }

    private static void __roze_rwlock_write_unlock(Object rw) {
        if (rw instanceof RozeRwLock) ((RozeRwLock) rw).writeUnlock();
    }

    private static boolean __roze_rwlock_try_read_lock(Object rw) {
        if (rw instanceof RozeRwLock) return ((RozeRwLock) rw).tryReadLock();
        return false;
    }

    private static boolean __roze_rwlock_try_write_lock(Object rw) {
        if (rw instanceof RozeRwLock) return ((RozeRwLock) rw).tryWriteLock();
        return false;
    }

    private static java.util.concurrent.atomic.AtomicBoolean __roze_atomic_bool_new(boolean init) {
        return new java.util.concurrent.atomic.AtomicBoolean(init);
    }
    private static boolean __roze_atomic_bool_load(Object a) {
        if (a instanceof java.util.concurrent.atomic.AtomicBoolean) return ((java.util.concurrent.atomic.AtomicBoolean) a).get();
        return false;
    }
    private static void __roze_atomic_bool_store(Object a, boolean val) {
        if (a instanceof java.util.concurrent.atomic.AtomicBoolean) ((java.util.concurrent.atomic.AtomicBoolean) a).set(val);
    }
    private static boolean __roze_atomic_bool_swap(Object a, boolean val) {
        if (a instanceof java.util.concurrent.atomic.AtomicBoolean) return ((java.util.concurrent.atomic.AtomicBoolean) a).getAndSet(val);
        return false;
    }
    private static boolean __roze_atomic_bool_cas(Object a, boolean exp, boolean des) {
        if (a instanceof java.util.concurrent.atomic.AtomicBoolean) return ((java.util.concurrent.atomic.AtomicBoolean) a).compareAndSet(exp, des);
        return false;
    }

    private static java.util.concurrent.atomic.AtomicLong __roze_atomic_i64_new(long init) {
        return new java.util.concurrent.atomic.AtomicLong(init);
    }
    private static int __roze_atomic_i64_load(Object a) {
        if (a instanceof java.util.concurrent.atomic.AtomicLong) return (int) ((java.util.concurrent.atomic.AtomicLong) a).get();
        return 0;
    }
    private static void __roze_atomic_i64_store(Object a, long val) {
        if (a instanceof java.util.concurrent.atomic.AtomicLong) ((java.util.concurrent.atomic.AtomicLong) a).set(val);
    }
    private static int __roze_atomic_i64_swap(Object a, long val) {
        if (a instanceof java.util.concurrent.atomic.AtomicLong) return (int) ((java.util.concurrent.atomic.AtomicLong) a).getAndSet(val);
        return 0;
    }
    private static boolean __roze_atomic_i64_cas(Object a, long exp, long des) {
        if (a instanceof java.util.concurrent.atomic.AtomicLong) return ((java.util.concurrent.atomic.AtomicLong) a).compareAndSet(exp, des);
        return false;
    }
    private static int __roze_atomic_i64_add(Object a, long delta) {
        if (a instanceof java.util.concurrent.atomic.AtomicLong) return (int) ((java.util.concurrent.atomic.AtomicLong) a).getAndAdd(delta);
        return 0;
    }
    private static int __roze_atomic_i64_sub(Object a, long delta) {
        if (a instanceof java.util.concurrent.atomic.AtomicLong) return (int) ((java.util.concurrent.atomic.AtomicLong) a).getAndAdd(-delta);
        return 0;
    }
    private static int __roze_atomic_i64_and(Object a, long val) {
        if (a instanceof java.util.concurrent.atomic.AtomicLong) return (int) ((java.util.concurrent.atomic.AtomicLong) a).getAndAccumulate(val, (x, y) -> x & y);
        return 0;
    }
    private static int __roze_atomic_i64_or(Object a, long val) {
        if (a instanceof java.util.concurrent.atomic.AtomicLong) return (int) ((java.util.concurrent.atomic.AtomicLong) a).getAndAccumulate(val, (x, y) -> x | y);
        return 0;
    }
    private static int __roze_atomic_i64_xor(Object a, long val) {
        if (a instanceof java.util.concurrent.atomic.AtomicLong) return (int) ((java.util.concurrent.atomic.AtomicLong) a).getAndAccumulate(val, (x, y) -> x ^ y);
        return 0;
    }

    private static java.util.concurrent.atomic.AtomicLong __roze_atomic_usize_new(long init) {
        return new java.util.concurrent.atomic.AtomicLong(init);
    }
    private static int __roze_atomic_usize_load(Object a) {
        return __roze_atomic_i64_load(a);
    }
    private static void __roze_atomic_usize_store(Object a, long val) {
        __roze_atomic_i64_store(a, val);
    }
    private static int __roze_atomic_usize_swap(Object a, long val) {
        return __roze_atomic_i64_swap(a, val);
    }
    private static boolean __roze_atomic_usize_cas(Object a, long exp, long des) {
        return __roze_atomic_i64_cas(a, exp, des);
    }
    private static int __roze_atomic_usize_add(Object a, long delta) {
        return __roze_atomic_i64_add(a, delta);
    }
    private static int __roze_atomic_usize_sub(Object a, long delta) {
        return __roze_atomic_i64_sub(a, delta);
    }

    private static java.util.concurrent.atomic.AtomicReference<Object> __roze_atomic_ptr_new(Object init) {
        return new java.util.concurrent.atomic.AtomicReference<>(init);
    }
    private static Object __roze_atomic_ptr_load(Object a) {
        if (a instanceof java.util.concurrent.atomic.AtomicReference) return ((java.util.concurrent.atomic.AtomicReference<?>) a).get();
        return null;
    }
    private static void __roze_atomic_ptr_store(Object a, Object val) {
        if (a instanceof java.util.concurrent.atomic.AtomicReference) ((java.util.concurrent.atomic.AtomicReference<Object>) a).set(val);
    }
    private static Object __roze_atomic_ptr_swap(Object a, Object val) {
        if (a instanceof java.util.concurrent.atomic.AtomicReference) return ((java.util.concurrent.atomic.AtomicReference<Object>) a).getAndSet(val);
        return null;
    }
    private static boolean __roze_atomic_ptr_cas(Object a, Object exp, Object des) {
        if (a instanceof java.util.concurrent.atomic.AtomicReference) {
            java.util.concurrent.atomic.AtomicReference<Object> ref = (java.util.concurrent.atomic.AtomicReference<Object>) a;
            Object cur = ref.get();
            if (java.util.Objects.equals(cur, exp)) {
                return ref.compareAndSet(cur, des);
            }
        }
        return false;
    }

    private static Object __roze_atomic_load(Object a) {
        if (a instanceof java.util.concurrent.atomic.AtomicBoolean) return ((java.util.concurrent.atomic.AtomicBoolean) a).get();
        if (a instanceof java.util.concurrent.atomic.AtomicLong) return (int) ((java.util.concurrent.atomic.AtomicLong) a).get();
        if (a instanceof java.util.concurrent.atomic.AtomicReference) return ((java.util.concurrent.atomic.AtomicReference<?>) a).get();
        return null;
    }
    private static void __roze_atomic_store(Object a, Object val) {
        if (a instanceof java.util.concurrent.atomic.AtomicBoolean) ((java.util.concurrent.atomic.AtomicBoolean) a).set(Boolean.TRUE.equals(val));
        else if (a instanceof java.util.concurrent.atomic.AtomicLong) ((java.util.concurrent.atomic.AtomicLong) a).set(val instanceof Number ? ((Number) val).longValue() : 0L);
        else if (a instanceof java.util.concurrent.atomic.AtomicReference) ((java.util.concurrent.atomic.AtomicReference<Object>) a).set(val);
    }
    private static Object __roze_atomic_swap(Object a, Object val) {
        if (a instanceof java.util.concurrent.atomic.AtomicBoolean) return ((java.util.concurrent.atomic.AtomicBoolean) a).getAndSet(Boolean.TRUE.equals(val));
        if (a instanceof java.util.concurrent.atomic.AtomicLong) return (int) ((java.util.concurrent.atomic.AtomicLong) a).getAndSet(val instanceof Number ? ((Number) val).longValue() : 0L);
        if (a instanceof java.util.concurrent.atomic.AtomicReference) return ((java.util.concurrent.atomic.AtomicReference<Object>) a).getAndSet(val);
        return null;
    }
    private static boolean __roze_atomic_cas(Object a, Object exp, Object des) {
        if (a instanceof java.util.concurrent.atomic.AtomicBoolean) return ((java.util.concurrent.atomic.AtomicBoolean) a).compareAndSet(Boolean.TRUE.equals(exp), Boolean.TRUE.equals(des));
        if (a instanceof java.util.concurrent.atomic.AtomicLong) return ((java.util.concurrent.atomic.AtomicLong) a).compareAndSet(exp instanceof Number ? ((Number) exp).longValue() : 0L, des instanceof Number ? ((Number) des).longValue() : 0L);
        if (a instanceof java.util.concurrent.atomic.AtomicReference) {
            java.util.concurrent.atomic.AtomicReference<Object> ref = (java.util.concurrent.atomic.AtomicReference<Object>) a;
            Object cur = ref.get();
            if (java.util.Objects.equals(cur, exp)) {
                return ref.compareAndSet(cur, des);
            }
        }
        return false;
    }
    private static int __roze_atomic_add(Object a, long delta) {
        return __roze_atomic_i64_add(a, delta);
    }
    private static int __roze_atomic_sub(Object a, long delta) {
        return __roze_atomic_i64_sub(a, delta);
    }

    // Structured Concurrency (M1-T07)
    private static RozeTaskGroup __roze_nursery_new() {
        return new RozeTaskGroup(0, 0);
    }
    private static RozeTaskGroup __roze_task_group_new() {
        return new RozeTaskGroup(0, 0);
    }
    private static RozeTaskGroup __roze_nursery_on_failure() {
        return new RozeTaskGroup(1, 0);
    }
    private static RozeTaskGroup __roze_task_group_on_failure() {
        return new RozeTaskGroup(1, 0);
    }
    private static RozeTaskGroup __roze_nursery_on_success() {
        return new RozeTaskGroup(2, 0);
    }
    private static RozeTaskGroup __roze_task_group_on_success() {
        return new RozeTaskGroup(2, 0);
    }
    private static RozeTaskGroup __roze_nursery_with_timeout(long ms) {
        return new RozeTaskGroup(0, ms);
    }
    private static RozeTaskGroup __roze_task_group_with_timeout(long ms) {
        return new RozeTaskGroup(0, ms);
    }

    private static RozeSubtask __roze_nursery_fork(Object tg, Object fnObj) {
        if (tg instanceof RozeTaskGroup && fnObj instanceof RozeFn) {
            return ((RozeTaskGroup) tg).fork((RozeFn) fnObj);
        }
        return null;
    }
    private static RozeSubtask __roze_task_group_fork(Object tg, Object fnObj) {
        return __roze_nursery_fork(tg, fnObj);
    }

    private static int __roze_nursery_join(Object tg) {
        if (tg instanceof RozeTaskGroup) {
            return ((RozeTaskGroup) tg).join();
        }
        return 0;
    }
    private static int __roze_task_group_join(Object tg) {
        return __roze_nursery_join(tg);
    }

    private static void __roze_nursery_cancel(Object tg) {
        if (tg instanceof RozeTaskGroup) {
            ((RozeTaskGroup) tg).cancel();
        }
    }
    private static void __roze_task_group_cancel(Object tg) {
        __roze_nursery_cancel(tg);
    }

    private static boolean __roze_nursery_is_cancelled(Object tg) {
        if (tg instanceof RozeTaskGroup) {
            return ((RozeTaskGroup) tg).isCancelled();
        }
        return false;
    }
    private static boolean __roze_task_group_is_cancelled(Object tg) {
        return __roze_nursery_is_cancelled(tg);
    }

    private static int __roze_nursery_result(Object tg) {
        if (tg instanceof RozeTaskGroup) {
            return ((RozeTaskGroup) tg).result();
        }
        return 0;
    }
    private static int __roze_task_group_result(Object tg) {
        return __roze_nursery_result(tg);
    }

    private static RozeCancellationToken __roze_nursery_token(Object tg) {
        if (tg instanceof RozeTaskGroup) {
            return ((RozeTaskGroup) tg).token();
        }
        return null;
    }
    private static RozeCancellationToken __roze_task_group_token(Object tg) {
        return __roze_nursery_token(tg);
    }

    private static int __roze_nursery_task_count(Object tg) {
        if (tg instanceof RozeTaskGroup) {
            return ((RozeTaskGroup) tg).taskCount();
        }
        return 0;
    }
    private static int __roze_task_group_task_count(Object tg) {
        return __roze_nursery_task_count(tg);
    }

    private static int __roze_nursery_completed_count(Object tg) {
        if (tg instanceof RozeTaskGroup) {
            return ((RozeTaskGroup) tg).completedCount();
        }
        return 0;
    }
    private static int __roze_task_group_completed_count(Object tg) {
        return __roze_nursery_completed_count(tg);
    }

    private static Object __roze_subtask_get(Object sub) {
        if (sub instanceof RozeSubtask) {
            return ((RozeSubtask) sub).get();
        }
        return null;
    }
    private static boolean __roze_subtask_is_done(Object sub) {
        if (sub instanceof RozeSubtask) {
            return ((RozeSubtask) sub).isDone();
        }
        return false;
    }
    private static boolean __roze_subtask_is_cancelled(Object sub) {
        if (sub instanceof RozeSubtask) {
            return ((RozeSubtask) sub).isCancelled();
        }
        return false;
    }

    private static RozeCancellationToken __roze_cancel_token_new() {
        return new RozeCancellationToken();
    }
    private static void __roze_cancel_token_cancel(Object tok) {
        if (tok instanceof RozeCancellationToken) {
            ((RozeCancellationToken) tok).cancel();
        }
    }
    private static boolean __roze_cancel_token_is_cancelled(Object tok) {
        if (tok instanceof RozeCancellationToken) {
            return ((RozeCancellationToken) tok).isCancelled();
        }
        return false;
    }
"#;

const DIRECT_MEMORY_HELPER_METHODS: &str = r#"
    private static final Object __ROZE_UNSAFE;
    private static final java.lang.reflect.Method __ROZE_ALLOC;
    private static final java.lang.reflect.Method __ROZE_REALLOC;
    private static final java.lang.reflect.Method __ROZE_FREE;
    private static final java.lang.reflect.Method __ROZE_GET_BYTE;
    private static final java.lang.reflect.Method __ROZE_PUT_BYTE;
    private static final java.lang.reflect.Method __ROZE_GET_LONG;
    private static final java.lang.reflect.Method __ROZE_PUT_LONG;
    private static final java.lang.reflect.Method __ROZE_SET_MEM;
    private static final java.lang.reflect.Method __ROZE_COPY_MEM;

    static {
        Object u = null;
        java.lang.reflect.Method alloc = null, realloc = null, free = null;
        java.lang.reflect.Method gb = null, pb = null, gl = null, pl = null;
        java.lang.reflect.Method sm = null, cm = null;
        try {
            Class<?> uc = Class.forName("sun.misc.Unsafe");
            java.lang.reflect.Field f = uc.getDeclaredField("theUnsafe");
            f.setAccessible(true);
            u = f.get(null);
            alloc = uc.getMethod("allocateMemory", long.class);
            realloc = uc.getMethod("reallocateMemory", long.class, long.class);
            free = uc.getMethod("freeMemory", long.class);
            gb = uc.getMethod("getByte", long.class);
            pb = uc.getMethod("putByte", long.class, byte.class);
            gl = uc.getMethod("getLong", long.class);
            pl = uc.getMethod("putLong", long.class, long.class);
            sm = uc.getMethod("setMemory", long.class, long.class, byte.class);
            cm = uc.getMethod("copyMemory", long.class, long.class, long.class);
        } catch (Throwable t) {}
        __ROZE_UNSAFE = u;
        __ROZE_ALLOC = alloc;
        __ROZE_REALLOC = realloc;
        __ROZE_FREE = free;
        __ROZE_GET_BYTE = gb;
        __ROZE_PUT_BYTE = pb;
        __ROZE_GET_LONG = gl;
        __ROZE_PUT_LONG = pl;
        __ROZE_SET_MEM = sm;
        __ROZE_COPY_MEM = cm;
    }

    private static long __roze_alloc(long size) {
        try {
            return (Long) __ROZE_ALLOC.invoke(__ROZE_UNSAFE, size);
        } catch (Exception e) {
            throw new RuntimeException("alloc failed: " + e.getMessage());
        }
    }

    private static long __roze_alloc_zeroed(long size) {
        long ptr = __roze_alloc(size);
        __roze_mem_set(ptr, 0, size);
        return ptr;
    }

    private static long __roze_realloc(long ptr, long new_size) {
        try {
            return (Long) __ROZE_REALLOC.invoke(__ROZE_UNSAFE, ptr, new_size);
        } catch (Exception e) {
            throw new RuntimeException("realloc failed: " + e.getMessage());
        }
    }

    private static void __roze_free(long ptr) {
        try {
            __ROZE_FREE.invoke(__ROZE_UNSAFE, ptr);
        } catch (Exception e) {
            throw new RuntimeException("free failed: " + e.getMessage());
        }
    }

    private static int __roze_ptr_read_u8(long ptr) {
        try {
            return ((Byte) __ROZE_GET_BYTE.invoke(__ROZE_UNSAFE, ptr)) & 0xFF;
        } catch (Exception e) {
            throw new RuntimeException("ptr_read_u8 failed: " + e.getMessage());
        }
    }

    private static int __roze_ptr_read(long ptr) {
        try {
            return (int) (long) (Long) __ROZE_GET_LONG.invoke(__ROZE_UNSAFE, ptr);
        } catch (Exception e) {
            throw new RuntimeException("ptr_read failed: " + e.getMessage());
        }
    }

    private static int __roze_ptr_read_u8_at(long ptr, long offset) {
        return __roze_ptr_read_u8(ptr + offset);
    }

    private static int __roze_ptr_read_at(long ptr, long offset) {
        return __roze_ptr_read(ptr + offset);
    }

    private static void __roze_ptr_write_u8(long ptr, int val) {
        try {
            __ROZE_PUT_BYTE.invoke(__ROZE_UNSAFE, ptr, (byte) val);
        } catch (Exception e) {
            throw new RuntimeException("ptr_write_u8 failed: " + e.getMessage());
        }
    }

    private static void __roze_ptr_write(long ptr, long val) {
        try {
            __ROZE_PUT_LONG.invoke(__ROZE_UNSAFE, ptr, val);
        } catch (Exception e) {
            throw new RuntimeException("ptr_write failed: " + e.getMessage());
        }
    }

    private static void __roze_ptr_write_u8_at(long ptr, long offset, int val) {
        __roze_ptr_write_u8(ptr + offset, val);
    }

    private static void __roze_ptr_write_at(long ptr, long offset, long val) {
        __roze_ptr_write(ptr + offset, val);
    }

    private static void __roze_mem_copy(long dest, long src, long count) {
        try {
            __ROZE_COPY_MEM.invoke(__ROZE_UNSAFE, src, dest, count);
        } catch (Exception e) {
            throw new RuntimeException("mem_copy failed: " + e.getMessage());
        }
    }

    private static void __roze_mem_set(long dest, int val, long count) {
        try {
            __ROZE_SET_MEM.invoke(__ROZE_UNSAFE, dest, count, (byte) val);
        } catch (Exception e) {
            throw new RuntimeException("mem_set failed: " + e.getMessage());
        }
    }

    private static void __roze_vga_clear() {
        System.out.print("\033[H\033[2J");
    }
    private static void __roze_vga_print(String s) {
        System.out.print(s);
    }
    private static void __roze_vga_write_char(long row, long col, long ch, long color) {
        System.out.print((char)ch);
    }
    private static void __roze_vga_write_byte(long offset, long byte_val) {}
    private static void __roze_vga_set_cursor(long row, long col) {}
    private static void __roze_vga_set_color(long fg, long bg) {}

    // Kernel & Multiboot2 (ADV-02)
    private static long __roze_multiboot2_get_magic() { return 0x36D76289L; }
    private static long __roze_multiboot2_get_info_ptr() { return 0L; }
    private static void __roze_outb(long port, long val) {}
    private static long __roze_inb(long port) { return 0L; }
    private static void __roze_qemu_exit(long code) {}
    private static void __roze_serial_print(String s) { System.out.print(s); }
    private static void __roze_serial_putchar(long ch) { System.out.print((char)ch); }
"#;

const NUMERIC_HELPER_METHODS: &str = r#"
    private static java.math.BigInteger __roze_to_i128(Object v) {
        if (v instanceof java.math.BigInteger) return (java.math.BigInteger)v;
        if (v instanceof Number) return java.math.BigInteger.valueOf(((Number)v).longValue());
        return new java.math.BigInteger(String.valueOf(v));
    }
    private static java.math.BigInteger __roze_to_u128(Object v) {
        if (v instanceof java.math.BigInteger) return (java.math.BigInteger)v;
        if (v instanceof Number) {
            long l = ((Number)v).longValue();
            if (l >= 0) return java.math.BigInteger.valueOf(l);
            return new java.math.BigInteger(Long.toUnsignedString(l));
        }
        return new java.math.BigInteger(String.valueOf(v));
    }

    // i8
    private static byte __roze_wrapping_add_i8(byte a, byte b) { return (byte)(a + b); }
    private static byte __roze_wrapping_sub_i8(byte a, byte b) { return (byte)(a - b); }
    private static byte __roze_wrapping_mul_i8(byte a, byte b) { return (byte)(a * b); }
    private static byte __roze_saturating_add_i8(byte a, byte b) {
        int r = a + b;
        if (r > 127) return 127;
        if (r < -128) return -128;
        return (byte)r;
    }
    private static byte __roze_saturating_sub_i8(byte a, byte b) {
        int r = a - b;
        if (r > 127) return 127;
        if (r < -128) return -128;
        return (byte)r;
    }
    private static byte __roze_saturating_mul_i8(byte a, byte b) {
        int r = a * b;
        if (r > 127) return 127;
        if (r < -128) return -128;
        return (byte)r;
    }

    // u8
    private static int __roze_wrapping_add_u8(int a, int b) { return (a + b) & 0xFF; }
    private static int __roze_wrapping_sub_u8(int a, int b) { return (a - b) & 0xFF; }
    private static int __roze_wrapping_mul_u8(int a, int b) { return (a * b) & 0xFF; }
    private static int __roze_saturating_add_u8(int a, int b) {
        int r = (a & 0xFF) + (b & 0xFF);
        return r > 255 ? 255 : r;
    }
    private static int __roze_saturating_sub_u8(int a, int b) {
        int r = (a & 0xFF) - (b & 0xFF);
        return r < 0 ? 0 : r;
    }
    private static int __roze_saturating_mul_u8(int a, int b) {
        int r = (a & 0xFF) * (b & 0xFF);
        return r > 255 ? 255 : r;
    }

    // i16
    private static short __roze_wrapping_add_i16(short a, short b) { return (short)(a + b); }
    private static short __roze_wrapping_sub_i16(short a, short b) { return (short)(a - b); }
    private static short __roze_wrapping_mul_i16(short a, short b) { return (short)(a * b); }
    private static short __roze_saturating_add_i16(short a, short b) {
        int r = a + b;
        if (r > 32767) return 32767;
        if (r < -32768) return -32768;
        return (short)r;
    }
    private static short __roze_saturating_sub_i16(short a, short b) {
        int r = a - b;
        if (r > 32767) return 32767;
        if (r < -32768) return -32768;
        return (short)r;
    }
    private static short __roze_saturating_mul_i16(short a, short b) {
        int r = a * b;
        if (r > 32767) return 32767;
        if (r < -32768) return -32768;
        return (short)r;
    }

    // u16
    private static int __roze_wrapping_add_u16(int a, int b) { return (a + b) & 0xFFFF; }
    private static int __roze_wrapping_sub_u16(int a, int b) { return (a - b) & 0xFFFF; }
    private static int __roze_wrapping_mul_u16(int a, int b) { return (a * b) & 0xFFFF; }
    private static int __roze_saturating_add_u16(int a, int b) {
        int r = (a & 0xFFFF) + (b & 0xFFFF);
        return r > 65535 ? 65535 : r;
    }
    private static int __roze_saturating_sub_u16(int a, int b) {
        int r = (a & 0xFFFF) - (b & 0xFFFF);
        return r < 0 ? 0 : r;
    }
    private static int __roze_saturating_mul_u16(int a, int b) {
        int r = (a & 0xFFFF) * (b & 0xFFFF);
        return r > 65535 ? 65535 : r;
    }

    // i32
    private static int __roze_wrapping_add_i32(int a, int b) { return a + b; }
    private static int __roze_wrapping_sub_i32(int a, int b) { return a - b; }
    private static int __roze_wrapping_mul_i32(int a, int b) { return a * b; }
    private static int __roze_saturating_add_i32(int a, int b) {
        long r = (long)a + (long)b;
        if (r > Integer.MAX_VALUE) return Integer.MAX_VALUE;
        if (r < Integer.MIN_VALUE) return Integer.MIN_VALUE;
        return (int)r;
    }
    private static int __roze_saturating_sub_i32(int a, int b) {
        long r = (long)a - (long)b;
        if (r > Integer.MAX_VALUE) return Integer.MAX_VALUE;
        if (r < Integer.MIN_VALUE) return Integer.MIN_VALUE;
        return (int)r;
    }
    private static int __roze_saturating_mul_i32(int a, int b) {
        long r = (long)a * (long)b;
        if (r > Integer.MAX_VALUE) return Integer.MAX_VALUE;
        if (r < Integer.MIN_VALUE) return Integer.MIN_VALUE;
        return (int)r;
    }

    // u32
    private static long __roze_wrapping_add_u32(long a, long b) { return (a + b) & 0xFFFFFFFFL; }
    private static long __roze_wrapping_sub_u32(long a, long b) { return (a - b) & 0xFFFFFFFFL; }
    private static long __roze_wrapping_mul_u32(long a, long b) { return (a * b) & 0xFFFFFFFFL; }
    private static long __roze_saturating_add_u32(long a, long b) {
        long r = (a & 0xFFFFFFFFL) + (b & 0xFFFFFFFFL);
        return r > 0xFFFFFFFFL ? 0xFFFFFFFFL : r;
    }
    private static long __roze_saturating_sub_u32(long a, long b) {
        long r = (a & 0xFFFFFFFFL) - (b & 0xFFFFFFFFL);
        return r < 0 ? 0L : r;
    }
    private static long __roze_saturating_mul_u32(long a, long b) {
        java.math.BigInteger r = java.math.BigInteger.valueOf(a & 0xFFFFFFFFL).multiply(java.math.BigInteger.valueOf(b & 0xFFFFFFFFL));
        java.math.BigInteger max = java.math.BigInteger.valueOf(0xFFFFFFFFL);
        return r.compareTo(max) > 0 ? 0xFFFFFFFFL : r.longValue();
    }

    // i64 / int / isize
    private static long __roze_wrapping_add_i64(long a, long b) { return a + b; }
    private static long __roze_wrapping_sub_i64(long a, long b) { return a - b; }
    private static long __roze_wrapping_mul_i64(long a, long b) { return a * b; }
    private static long __roze_saturating_add_i64(long a, long b) {
        java.math.BigInteger r = java.math.BigInteger.valueOf(a).add(java.math.BigInteger.valueOf(b));
        java.math.BigInteger max = java.math.BigInteger.valueOf(Long.MAX_VALUE);
        java.math.BigInteger min = java.math.BigInteger.valueOf(Long.MIN_VALUE);
        if (r.compareTo(max) > 0) return Long.MAX_VALUE;
        if (r.compareTo(min) < 0) return Long.MIN_VALUE;
        return r.longValue();
    }
    private static long __roze_saturating_sub_i64(long a, long b) {
        java.math.BigInteger r = java.math.BigInteger.valueOf(a).subtract(java.math.BigInteger.valueOf(b));
        java.math.BigInteger max = java.math.BigInteger.valueOf(Long.MAX_VALUE);
        java.math.BigInteger min = java.math.BigInteger.valueOf(Long.MIN_VALUE);
        if (r.compareTo(max) > 0) return Long.MAX_VALUE;
        if (r.compareTo(min) < 0) return Long.MIN_VALUE;
        return r.longValue();
    }
    private static long __roze_saturating_mul_i64(long a, long b) {
        java.math.BigInteger r = java.math.BigInteger.valueOf(a).multiply(java.math.BigInteger.valueOf(b));
        java.math.BigInteger max = java.math.BigInteger.valueOf(Long.MAX_VALUE);
        java.math.BigInteger min = java.math.BigInteger.valueOf(Long.MIN_VALUE);
        if (r.compareTo(max) > 0) return Long.MAX_VALUE;
        if (r.compareTo(min) < 0) return Long.MIN_VALUE;
        return r.longValue();
    }

    // u64 / usize
    private static long __roze_wrapping_add_u64(long a, long b) { return a + b; }
    private static long __roze_wrapping_sub_u64(long a, long b) { return a - b; }
    private static long __roze_wrapping_mul_u64(long a, long b) { return a * b; }
    private static long __roze_saturating_add_u64(long a, long b) {
        java.math.BigInteger ua = new java.math.BigInteger(Long.toUnsignedString(a));
        java.math.BigInteger ub = new java.math.BigInteger(Long.toUnsignedString(b));
        java.math.BigInteger sum = ua.add(ub);
        java.math.BigInteger max = java.math.BigInteger.valueOf(2).pow(64).subtract(java.math.BigInteger.ONE);
        return sum.compareTo(max) >= 0 ? -1L : sum.longValue();
    }
    private static long __roze_saturating_sub_u64(long a, long b) {
        java.math.BigInteger ua = new java.math.BigInteger(Long.toUnsignedString(a));
        java.math.BigInteger ub = new java.math.BigInteger(Long.toUnsignedString(b));
        if (ua.compareTo(ub) <= 0) return 0L;
        return ua.subtract(ub).longValue();
    }
    private static long __roze_saturating_mul_u64(long a, long b) {
        java.math.BigInteger ua = new java.math.BigInteger(Long.toUnsignedString(a));
        java.math.BigInteger ub = new java.math.BigInteger(Long.toUnsignedString(b));
        java.math.BigInteger prod = ua.multiply(ub);
        java.math.BigInteger max = java.math.BigInteger.valueOf(2).pow(64).subtract(java.math.BigInteger.ONE);
        return prod.compareTo(max) >= 0 ? -1L : prod.longValue();
    }

    // i128
    private static final java.math.BigInteger __ROZE_I128_MIN = new java.math.BigInteger("-170141183460469231731687303715884105728");
    private static final java.math.BigInteger __ROZE_I128_MAX = new java.math.BigInteger("170141183460469231731687303715884105727");
    private static final java.math.BigInteger __ROZE_U128_MAX = new java.math.BigInteger("340282366920938463463374607431768211455");
    private static final java.math.BigInteger __ROZE_2_POW_128 = java.math.BigInteger.valueOf(2).pow(128);

    private static java.math.BigInteger __roze_wrapping_add_i128(java.math.BigInteger a, java.math.BigInteger b) {
        java.math.BigInteger r = a.add(b).remainder(__ROZE_2_POW_128);
        if (r.compareTo(__ROZE_I128_MAX) > 0) r = r.subtract(__ROZE_2_POW_128);
        else if (r.compareTo(__ROZE_I128_MIN) < 0) r = r.add(__ROZE_2_POW_128);
        return r;
    }
    private static java.math.BigInteger __roze_wrapping_sub_i128(java.math.BigInteger a, java.math.BigInteger b) {
        java.math.BigInteger r = a.subtract(b).remainder(__ROZE_2_POW_128);
        if (r.compareTo(__ROZE_I128_MAX) > 0) r = r.subtract(__ROZE_2_POW_128);
        else if (r.compareTo(__ROZE_I128_MIN) < 0) r = r.add(__ROZE_2_POW_128);
        return r;
    }
    private static java.math.BigInteger __roze_wrapping_mul_i128(java.math.BigInteger a, java.math.BigInteger b) {
        java.math.BigInteger r = a.multiply(b).remainder(__ROZE_2_POW_128);
        if (r.compareTo(__ROZE_I128_MAX) > 0) r = r.subtract(__ROZE_2_POW_128);
        else if (r.compareTo(__ROZE_I128_MIN) < 0) r = r.add(__ROZE_2_POW_128);
        return r;
    }
    private static java.math.BigInteger __roze_saturating_add_i128(java.math.BigInteger a, java.math.BigInteger b) {
        java.math.BigInteger r = a.add(b);
        if (r.compareTo(__ROZE_I128_MAX) > 0) return __ROZE_I128_MAX;
        if (r.compareTo(__ROZE_I128_MIN) < 0) return __ROZE_I128_MIN;
        return r;
    }
    private static java.math.BigInteger __roze_saturating_sub_i128(java.math.BigInteger a, java.math.BigInteger b) {
        java.math.BigInteger r = a.subtract(b);
        if (r.compareTo(__ROZE_I128_MAX) > 0) return __ROZE_I128_MAX;
        if (r.compareTo(__ROZE_I128_MIN) < 0) return __ROZE_I128_MIN;
        return r;
    }
    private static java.math.BigInteger __roze_saturating_mul_i128(java.math.BigInteger a, java.math.BigInteger b) {
        java.math.BigInteger r = a.multiply(b);
        if (r.compareTo(__ROZE_I128_MAX) > 0) return __ROZE_I128_MAX;
        if (r.compareTo(__ROZE_I128_MIN) < 0) return __ROZE_I128_MIN;
        return r;
    }

    // u128
    private static java.math.BigInteger __roze_wrapping_add_u128(java.math.BigInteger a, java.math.BigInteger b) {
        return a.add(b).mod(__ROZE_2_POW_128);
    }
    private static java.math.BigInteger __roze_wrapping_sub_u128(java.math.BigInteger a, java.math.BigInteger b) {
        return a.subtract(b).mod(__ROZE_2_POW_128);
    }
    private static java.math.BigInteger __roze_wrapping_mul_u128(java.math.BigInteger a, java.math.BigInteger b) {
        return a.multiply(b).mod(__ROZE_2_POW_128);
    }
    private static java.math.BigInteger __roze_saturating_add_u128(java.math.BigInteger a, java.math.BigInteger b) {
        java.math.BigInteger r = a.add(b);
        return r.compareTo(__ROZE_U128_MAX) > 0 ? __ROZE_U128_MAX : r;
    }
    private static java.math.BigInteger __roze_saturating_sub_u128(java.math.BigInteger a, java.math.BigInteger b) {
        if (a.compareTo(b) <= 0) return java.math.BigInteger.ZERO;
        return a.subtract(b);
    }
    private static java.math.BigInteger __roze_saturating_mul_u128(java.math.BigInteger a, java.math.BigInteger b) {
        java.math.BigInteger r = a.multiply(b);
        return r.compareTo(__ROZE_U128_MAX) > 0 ? __ROZE_U128_MAX : r;
    }
    private static java.math.BigInteger __roze_i128_min() { return __ROZE_I128_MIN; }
    private static java.math.BigInteger __roze_i128_max() { return __ROZE_I128_MAX; }
    private static java.math.BigInteger __roze_u128_min() { return java.math.BigInteger.ZERO; }
    private static java.math.BigInteger __roze_u128_max() { return __ROZE_U128_MAX; }
"#;

const SIMD_HELPER_METHODS: &str = r#"
    private static RozeVec4f __roze_vec4f_load(long ptr) {
        float f0 = Float.intBitsToFloat((int)__roze_ptr_read(ptr));
        float f1 = Float.intBitsToFloat((int)__roze_ptr_read_at(ptr, 4));
        float f2 = Float.intBitsToFloat((int)__roze_ptr_read_at(ptr, 8));
        float f3 = Float.intBitsToFloat((int)__roze_ptr_read_at(ptr, 12));
        return new RozeVec4f(f0, f1, f2, f3);
    }
    private static void __roze_vec4f_store(long ptr, RozeVec4f v) {
        if (v == null) return;
        __roze_ptr_write_at(ptr, 0, Float.floatToRawIntBits(v.f0));
        __roze_ptr_write_at(ptr, 4, Float.floatToRawIntBits(v.f1));
        __roze_ptr_write_at(ptr, 8, Float.floatToRawIntBits(v.f2));
        __roze_ptr_write_at(ptr, 12, Float.floatToRawIntBits(v.f3));
    }

    private static RozeVec8f __roze_vec8f_load(long ptr) {
        float[] a = new float[8];
        for (int i = 0; i < 8; i++) a[i] = Float.intBitsToFloat((int)__roze_ptr_read_at(ptr, i * 4));
        return new RozeVec8f(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]);
    }
    private static void __roze_vec8f_store(long ptr, RozeVec8f v) {
        if (v == null) return;
        for (int i = 0; i < 8; i++) __roze_ptr_write_at(ptr, i * 4, Float.floatToRawIntBits(v.get(i)));
    }

    private static RozeVec4i __roze_vec4i_load(long ptr) {
        return new RozeVec4i(
            (int)__roze_ptr_read_at(ptr, 0),
            (int)__roze_ptr_read_at(ptr, 4),
            (int)__roze_ptr_read_at(ptr, 8),
            (int)__roze_ptr_read_at(ptr, 12)
        );
    }
    private static void __roze_vec4i_store(long ptr, RozeVec4i v) {
        if (v == null) return;
        __roze_ptr_write_at(ptr, 0, v.i0);
        __roze_ptr_write_at(ptr, 4, v.i1);
        __roze_ptr_write_at(ptr, 8, v.i2);
        __roze_ptr_write_at(ptr, 12, v.i3);
    }

    private static RozeVec16u8 __roze_vec16u8_load(long ptr) {
        RozeVec16u8 v = new RozeVec16u8();
        for (int i = 0; i < 16; i++) v.data[i] = (byte)__roze_ptr_read_u8(ptr + i);
        return v;
    }
    private static void __roze_vec16u8_store(long ptr, RozeVec16u8 v) {
        if (v == null) return;
        for (int i = 0; i < 16; i++) __roze_ptr_write_u8(ptr + i, v.data[i] & 0xFF);
    }
"#;

const SIMD_HELPER_CLASSES: &str = r#"
class RozeVec4f {
    public float f0, f1, f2, f3;
    public RozeVec4f(float f0, float f1, float f2, float f3) {
        this.f0 = f0; this.f1 = f1; this.f2 = f2; this.f3 = f3;
    }
    public static RozeVec4f splat(float v) { return new RozeVec4f(v, v, v, v); }
    public static RozeVec4f zero() { return new RozeVec4f(0f, 0f, 0f, 0f); }
    public RozeVec4f add(RozeVec4f b) { return new RozeVec4f(f0 + b.f0, f1 + b.f1, f2 + b.f2, f3 + b.f3); }
    public RozeVec4f add(float v) { return new RozeVec4f(f0 + v, f1 + v, f2 + v, f3 + v); }
    public RozeVec4f sub(RozeVec4f b) { return new RozeVec4f(f0 - b.f0, f1 - b.f1, f2 - b.f2, f3 - b.f3); }
    public RozeVec4f sub(float v) { return new RozeVec4f(f0 - v, f1 - v, f2 - v, f3 - v); }
    public RozeVec4f mul(RozeVec4f b) { return new RozeVec4f(f0 * b.f0, f1 * b.f1, f2 * b.f2, f3 * b.f3); }
    public RozeVec4f mul(float v) { return new RozeVec4f(f0 * v, f1 * v, f2 * v, f3 * v); }
    public RozeVec4f div(RozeVec4f b) { return new RozeVec4f(f0 / b.f0, f1 / b.f1, f2 / b.f2, f3 / b.f3); }
    public RozeVec4f div(float v) { return new RozeVec4f(f0 / v, f1 / v, f2 / v, f3 / v); }
    public float dot(RozeVec4f b) { return (f0 * b.f0) + (f1 * b.f1) + (f2 * b.f2) + (f3 * b.f3); }
    public float sum() { return f0 + f1 + f2 + f3; }
    public RozeVec4f min(RozeVec4f b) { return new RozeVec4f(Math.min(f0, b.f0), Math.min(f1, b.f1), Math.min(f2, b.f2), Math.min(f3, b.f3)); }
    public RozeVec4f max(RozeVec4f b) { return new RozeVec4f(Math.max(f0, b.f0), Math.max(f1, b.f1), Math.max(f2, b.f2), Math.max(f3, b.f3)); }
    public RozeVec4f sqrt() { return new RozeVec4f((float)Math.sqrt(f0), (float)Math.sqrt(f1), (float)Math.sqrt(f2), (float)Math.sqrt(f3)); }
    public RozeVec4f abs() { return new RozeVec4f(Math.abs(f0), Math.abs(f1), Math.abs(f2), Math.abs(f3)); }
    public RozeVec4f fma(RozeVec4f b, RozeVec4f c) { return new RozeVec4f((f0 * b.f0) + c.f0, (f1 * b.f1) + c.f1, (f2 * b.f2) + c.f2, (f3 * b.f3) + c.f3); }
    public float get(long idx) {
        int i = (int)idx;
        if (i == 0) return f0;
        if (i == 1) return f1;
        if (i == 2) return f2;
        if (i == 3) return f3;
        return 0f;
    }
    public RozeVec4f set(long idx, float v) {
        float a0 = f0, a1 = f1, a2 = f2, a3 = f3;
        int i = (int)idx;
        if (i == 0) a0 = v;
        else if (i == 1) a1 = v;
        else if (i == 2) a2 = v;
        else if (i == 3) a3 = v;
        return new RozeVec4f(a0, a1, a2, a3);
    }
    @Override
    public boolean equals(Object o) {
        if (!(o instanceof RozeVec4f)) return false;
        RozeVec4f b = (RozeVec4f)o;
        return f0 == b.f0 && f1 == b.f1 && f2 == b.f2 && f3 == b.f3;
    }
    @Override
    public String toString() {
        return "Vec4f(" + fmtFloat(f0) + ", " + fmtFloat(f1) + ", " + fmtFloat(f2) + ", " + fmtFloat(f3) + ")";
    }
    private static String fmtFloat(float x) {
        if (x == (long)x) return ((long)x) + ".0";
        return String.valueOf(x);
    }
}

class RozeVec8f {
    public float f0, f1, f2, f3, f4, f5, f6, f7;
    public RozeVec8f(float f0, float f1, float f2, float f3, float f4, float f5, float f6, float f7) {
        this.f0 = f0; this.f1 = f1; this.f2 = f2; this.f3 = f3;
        this.f4 = f4; this.f5 = f5; this.f6 = f6; this.f7 = f7;
    }
    public static RozeVec8f splat(float v) { return new RozeVec8f(v, v, v, v, v, v, v, v); }
    public static RozeVec8f zero() { return new RozeVec8f(0f, 0f, 0f, 0f, 0f, 0f, 0f, 0f); }
    public RozeVec8f add(RozeVec8f b) {
        return new RozeVec8f(f0 + b.f0, f1 + b.f1, f2 + b.f2, f3 + b.f3, f4 + b.f4, f5 + b.f5, f6 + b.f6, f7 + b.f7);
    }
    public RozeVec8f add(float v) {
        return new RozeVec8f(f0 + v, f1 + v, f2 + v, f3 + v, f4 + v, f5 + v, f6 + v, f7 + v);
    }
    public RozeVec8f sub(RozeVec8f b) {
        return new RozeVec8f(f0 - b.f0, f1 - b.f1, f2 - b.f2, f3 - b.f3, f4 - b.f4, f5 - b.f5, f6 - b.f6, f7 - b.f7);
    }
    public RozeVec8f sub(float v) {
        return new RozeVec8f(f0 - v, f1 - v, f2 - v, f3 - v, f4 - v, f5 - v, f6 - v, f7 - v);
    }
    public RozeVec8f mul(RozeVec8f b) {
        return new RozeVec8f(f0 * b.f0, f1 * b.f1, f2 * b.f2, f3 * b.f3, f4 * b.f4, f5 * b.f5, f6 * b.f6, f7 * b.f7);
    }
    public RozeVec8f mul(float v) {
        return new RozeVec8f(f0 * v, f1 * v, f2 * v, f3 * v, f4 * v, f5 * v, f6 * v, f7 * v);
    }
    public RozeVec8f div(RozeVec8f b) {
        return new RozeVec8f(f0 / b.f0, f1 / b.f1, f2 / b.f2, f3 / b.f3, f4 / b.f4, f5 / b.f5, f6 / b.f6, f7 / b.f7);
    }
    public RozeVec8f div(float v) {
        return new RozeVec8f(f0 / v, f1 / v, f2 / v, f3 / v, f4 / v, f5 / v, f6 / v, f7 / v);
    }
    public float dot(RozeVec8f b) {
        return (f0 * b.f0) + (f1 * b.f1) + (f2 * b.f2) + (f3 * b.f3) + (f4 * b.f4) + (f5 * b.f5) + (f6 * b.f6) + (f7 * b.f7);
    }
    public float sum() {
        return f0 + f1 + f2 + f3 + f4 + f5 + f6 + f7;
    }
    public RozeVec8f min(RozeVec8f b) {
        return new RozeVec8f(Math.min(f0, b.f0), Math.min(f1, b.f1), Math.min(f2, b.f2), Math.min(f3, b.f3),
                             Math.min(f4, b.f4), Math.min(f5, b.f5), Math.min(f6, b.f6), Math.min(f7, b.f7));
    }
    public RozeVec8f max(RozeVec8f b) {
        return new RozeVec8f(Math.max(f0, b.f0), Math.max(f1, b.f1), Math.max(f2, b.f2), Math.max(f3, b.f3),
                             Math.max(f4, b.f4), Math.max(f5, b.f5), Math.max(f6, b.f6), Math.max(f7, b.f7));
    }
    public RozeVec8f sqrt() {
        return new RozeVec8f((float)Math.sqrt(f0), (float)Math.sqrt(f1), (float)Math.sqrt(f2), (float)Math.sqrt(f3),
                             (float)Math.sqrt(f4), (float)Math.sqrt(f5), (float)Math.sqrt(f6), (float)Math.sqrt(f7));
    }
    public RozeVec8f abs() {
        return new RozeVec8f(Math.abs(f0), Math.abs(f1), Math.abs(f2), Math.abs(f3),
                             Math.abs(f4), Math.abs(f5), Math.abs(f6), Math.abs(f7));
    }
    public RozeVec8f fma(RozeVec8f b, RozeVec8f c) {
        return new RozeVec8f((f0 * b.f0) + c.f0, (f1 * b.f1) + c.f1, (f2 * b.f2) + c.f2, (f3 * b.f3) + c.f3,
                             (f4 * b.f4) + c.f4, (f5 * b.f5) + c.f5, (f6 * b.f6) + c.f6, (f7 * b.f7) + c.f7);
    }
    public float get(long idx) {
        int i = (int)idx;
        switch (i) {
            case 0: return f0;
            case 1: return f1;
            case 2: return f2;
            case 3: return f3;
            case 4: return f4;
            case 5: return f5;
            case 6: return f6;
            case 7: return f7;
            default: return 0f;
        }
    }
    public RozeVec8f set(long idx, float v) {
        float[] a = new float[]{f0, f1, f2, f3, f4, f5, f6, f7};
        int i = (int)idx;
        if (i >= 0 && i < 8) a[i] = v;
        return new RozeVec8f(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]);
    }
    @Override
    public boolean equals(Object o) {
        if (!(o instanceof RozeVec8f)) return false;
        RozeVec8f b = (RozeVec8f)o;
        return f0 == b.f0 && f1 == b.f1 && f2 == b.f2 && f3 == b.f3 &&
               f4 == b.f4 && f5 == b.f5 && f6 == b.f6 && f7 == b.f7;
    }
    @Override
    public String toString() {
        return "Vec8f(" + fmtFloat(f0) + ", " + fmtFloat(f1) + ", " + fmtFloat(f2) + ", " + fmtFloat(f3) + ", " +
                          fmtFloat(f4) + ", " + fmtFloat(f5) + ", " + fmtFloat(f6) + ", " + fmtFloat(f7) + ")";
    }
    private static String fmtFloat(float x) {
        if (x == (long)x) return ((long)x) + ".0";
        return String.valueOf(x);
    }
}

class RozeVec4i {
    public int i0, i1, i2, i3;
    public RozeVec4i(int i0, int i1, int i2, int i3) {
        this.i0 = i0; this.i1 = i1; this.i2 = i2; this.i3 = i3;
    }
    public static RozeVec4i splat(int v) { return new RozeVec4i(v, v, v, v); }
    public static RozeVec4i zero() { return new RozeVec4i(0, 0, 0, 0); }
    public RozeVec4i add(RozeVec4i b) { return new RozeVec4i(i0 + b.i0, i1 + b.i1, i2 + b.i2, i3 + b.i3); }
    public RozeVec4i add(int v) { return new RozeVec4i(i0 + v, i1 + v, i2 + v, i3 + v); }
    public RozeVec4i sub(RozeVec4i b) { return new RozeVec4i(i0 - b.i0, i1 - b.i1, i2 - b.i2, i3 - b.i3); }
    public RozeVec4i sub(int v) { return new RozeVec4i(i0 - v, i1 - v, i2 - v, i3 - v); }
    public RozeVec4i mul(RozeVec4i b) { return new RozeVec4i(i0 * b.i0, i1 * b.i1, i2 * b.i2, i3 * b.i3); }
    public RozeVec4i mul(int v) { return new RozeVec4i(i0 * v, i1 * v, i2 * v, i3 * v); }
    public RozeVec4i div(RozeVec4i b) { return new RozeVec4i(b.i0 != 0 ? i0 / b.i0 : 0, b.i1 != 0 ? i1 / b.i1 : 0, b.i2 != 0 ? i2 / b.i2 : 0, b.i3 != 0 ? i3 / b.i3 : 0); }
    public RozeVec4i div(int v) { return new RozeVec4i(v != 0 ? i0 / v : 0, v != 0 ? i1 / v : 0, v != 0 ? i2 / v : 0, v != 0 ? i3 / v : 0); }
    public int sum() { return i0 + i1 + i2 + i3; }
    public RozeVec4i min(RozeVec4i b) { return new RozeVec4i(Math.min(i0, b.i0), Math.min(i1, b.i1), Math.min(i2, b.i2), Math.min(i3, b.i3)); }
    public RozeVec4i max(RozeVec4i b) { return new RozeVec4i(Math.max(i0, b.i0), Math.max(i1, b.i1), Math.max(i2, b.i2), Math.max(i3, b.i3)); }
    public RozeVec4i abs() { return new RozeVec4i(Math.abs(i0), Math.abs(i1), Math.abs(i2), Math.abs(i3)); }
    public int get(long idx) {
        int i = (int)idx;
        if (i == 0) return i0;
        if (i == 1) return i1;
        if (i == 2) return i2;
        if (i == 3) return i3;
        return 0;
    }
    public RozeVec4i set(long idx, int v) {
        int a0 = i0, a1 = i1, a2 = i2, a3 = i3;
        int i = (int)idx;
        if (i == 0) a0 = v;
        else if (i == 1) a1 = v;
        else if (i == 2) a2 = v;
        else if (i == 3) a3 = v;
        return new RozeVec4i(a0, a1, a2, a3);
    }
    @Override
    public boolean equals(Object o) {
        if (!(o instanceof RozeVec4i)) return false;
        RozeVec4i b = (RozeVec4i)o;
        return i0 == b.i0 && i1 == b.i1 && i2 == b.i2 && i3 == b.i3;
    }
    @Override
    public String toString() {
        return "Vec4i(" + i0 + ", " + i1 + ", " + i2 + ", " + i3 + ")";
    }
}

class RozeVec16u8 {
    public byte[] data = new byte[16];
    public RozeVec16u8(int... bytes) {
        for (int i = 0; i < Math.min(bytes.length, 16); i++) {
            this.data[i] = (byte)bytes[i];
        }
    }
    public static RozeVec16u8 splat(int v) {
        RozeVec16u8 res = new RozeVec16u8();
        byte b = (byte)v;
        for (int i = 0; i < 16; i++) res.data[i] = b;
        return res;
    }
    public static RozeVec16u8 zero() { return new RozeVec16u8(); }
    public RozeVec16u8 add(RozeVec16u8 b) {
        RozeVec16u8 res = new RozeVec16u8();
        for (int i = 0; i < 16; i++) res.data[i] = (byte)(data[i] + b.data[i]);
        return res;
    }
    public RozeVec16u8 add(int v) {
        RozeVec16u8 res = new RozeVec16u8();
        for (int i = 0; i < 16; i++) res.data[i] = (byte)(data[i] + v);
        return res;
    }
    public RozeVec16u8 sub(RozeVec16u8 b) {
        RozeVec16u8 res = new RozeVec16u8();
        for (int i = 0; i < 16; i++) res.data[i] = (byte)(data[i] - b.data[i]);
        return res;
    }
    public RozeVec16u8 sub(int v) {
        RozeVec16u8 res = new RozeVec16u8();
        for (int i = 0; i < 16; i++) res.data[i] = (byte)(data[i] - v);
        return res;
    }
    public RozeVec16u8 mul(RozeVec16u8 b) {
        RozeVec16u8 res = new RozeVec16u8();
        for (int i = 0; i < 16; i++) res.data[i] = (byte)(data[i] * b.data[i]);
        return res;
    }
    public RozeVec16u8 mul(int v) {
        RozeVec16u8 res = new RozeVec16u8();
        for (int i = 0; i < 16; i++) res.data[i] = (byte)(data[i] * v);
        return res;
    }
    public RozeVec16u8 div(RozeVec16u8 b) {
        RozeVec16u8 res = new RozeVec16u8();
        for (int i = 0; i < 16; i++) {
            int denom = b.data[i] & 0xFF;
            res.data[i] = (byte)(denom != 0 ? ((data[i] & 0xFF) / denom) : 0);
        }
        return res;
    }
    public RozeVec16u8 div(int v) {
        RozeVec16u8 res = new RozeVec16u8();
        for (int i = 0; i < 16; i++) res.data[i] = (byte)(v != 0 ? ((data[i] & 0xFF) / v) : 0);
        return res;
    }
    public RozeVec16u8 saturating_add(RozeVec16u8 b) {
        RozeVec16u8 res = new RozeVec16u8();
        for (int i = 0; i < 16; i++) {
            int r = (data[i] & 0xFF) + (b.data[i] & 0xFF);
            res.data[i] = (byte)(r > 255 ? 255 : r);
        }
        return res;
    }
    public RozeVec16u8 saturating_sub(RozeVec16u8 b) {
        RozeVec16u8 res = new RozeVec16u8();
        for (int i = 0; i < 16; i++) {
            int r = (data[i] & 0xFF) - (b.data[i] & 0xFF);
            res.data[i] = (byte)(r < 0 ? 0 : r);
        }
        return res;
    }
    public long sum() {
        long s = 0;
        for (int i = 0; i < 16; i++) s += (data[i] & 0xFF);
        return s;
    }
    public RozeVec16u8 min(RozeVec16u8 b) {
        RozeVec16u8 res = new RozeVec16u8();
        for (int i = 0; i < 16; i++) res.data[i] = (byte)Math.min(data[i] & 0xFF, b.data[i] & 0xFF);
        return res;
    }
    public RozeVec16u8 max(RozeVec16u8 b) {
        RozeVec16u8 res = new RozeVec16u8();
        for (int i = 0; i < 16; i++) res.data[i] = (byte)Math.max(data[i] & 0xFF, b.data[i] & 0xFF);
        return res;
    }
    public int get(long idx) {
        int i = (int)idx;
        if (i >= 0 && i < 16) return data[i] & 0xFF;
        return 0;
    }
    public RozeVec16u8 set(long idx, int val) {
        RozeVec16u8 res = new RozeVec16u8();
        System.arraycopy(data, 0, res.data, 0, 16);
        int i = (int)idx;
        if (i >= 0 && i < 16) res.data[i] = (byte)val;
        return res;
    }
    @Override
    public boolean equals(Object o) {
        if (!(o instanceof RozeVec16u8)) return false;
        RozeVec16u8 b = (RozeVec16u8)o;
        return java.util.Arrays.equals(data, b.data);
    }
    @Override
    public String toString() {
        StringBuilder sb = new StringBuilder("Vec16u8(");
        for (int i = 0; i < 16; i++) {
            sb.append(data[i] & 0xFF);
            if (i < 15) sb.append(", ");
        }
        sb.append(")");
        return sb.toString();
    }
}
"#;

// ==================== Collections Helper Classes (M2-T03) ====================
const COLLECTIONS_HELPER_CLASSES: &str = r#"
class RozeVecDeque {
    private final java.util.ArrayDeque<Object> deque;

    public RozeVecDeque() {
        this.deque = new java.util.ArrayDeque<>();
    }

    public RozeVecDeque(int capacity) {
        this.deque = new java.util.ArrayDeque<>(Math.max(capacity, 8));
    }

    public void pushBack(Object val) { deque.addLast(val); }
    public void pushFront(Object val) { deque.addFirst(val); }
    public Object popBack() { return deque.isEmpty() ? null : deque.removeLast(); }
    public Object popFront() { return deque.isEmpty() ? null : deque.removeFirst(); }
    public Object front() { return deque.peekFirst(); }
    public Object back() { return deque.peekLast(); }
    public Object get(int idx) {
        if (idx < 0 || idx >= deque.size()) return null;
        int i = 0;
        for (Object item : deque) {
            if (i == idx) return item;
            i++;
        }
        return null;
    }
    public void set(int idx, Object val) {
        if (idx < 0 || idx >= deque.size()) return;
        java.util.ArrayList<Object> list = new java.util.ArrayList<>(deque);
        list.set(idx, val);
        deque.clear();
        deque.addAll(list);
    }
    public long len() { return deque.size(); }
    public boolean isEmpty() { return deque.isEmpty(); }
    public void clear() { deque.clear(); }

    @Override
    public String toString() {
        return "VecDeque" + deque.toString();
    }
}

class RozeBTreeMap {
    private final java.util.TreeMap<Object, Object> map;

    public RozeBTreeMap() {
        this.map = new java.util.TreeMap<>();
    }

    public void insert(Object key, Object value) { map.put(key, value); }
    public Object get(Object key) { return map.get(key); }
    public boolean containsKey(Object key) { return map.containsKey(key); }
    public Object remove(Object key) { return map.remove(key); }
    public long len() { return map.size(); }
    public boolean isEmpty() { return map.isEmpty(); }
    public Object firstKey() { return map.isEmpty() ? null : map.firstKey(); }
    public Object lastKey() { return map.isEmpty() ? null : map.lastKey(); }
    public java.util.List<Object> keys() { return new java.util.ArrayList<>(map.keySet()); }
    public java.util.List<Object> values() { return new java.util.ArrayList<>(map.values()); }
    public void clear() { map.clear(); }

    @Override
    public String toString() {
        return "BTreeMap" + map.toString();
    }
}

class RozeBTreeSet {
    private final java.util.TreeSet<Object> set;

    public RozeBTreeSet() {
        this.set = new java.util.TreeSet<>();
    }

    public boolean insert(Object val) { return set.add(val); }
    public boolean contains(Object val) { return set.contains(val); }
    public boolean remove(Object val) { return set.remove(val); }
    public long len() { return set.size(); }
    public boolean isEmpty() { return set.isEmpty(); }
    public Object first() { return set.isEmpty() ? null : set.first(); }
    public Object last() { return set.isEmpty() ? null : set.last(); }
    public java.util.List<Object> toList() { return new java.util.ArrayList<>(set); }
    public void clear() { set.clear(); }

    @Override
    public String toString() {
        return "BTreeSet" + set.toString();
    }
}

class RozeBinaryHeap {
    private final java.util.PriorityQueue<Object> pq;

    @SuppressWarnings("unchecked")
    public RozeBinaryHeap() {
        this.pq = new java.util.PriorityQueue<>((a, b) -> ((Comparable) b).compareTo(a));
    }

    public void push(Object val) { pq.offer(val); }
    public Object pop() { return pq.poll(); }
    public Object peek() { return pq.peek(); }
    public long len() { return pq.size(); }
    public boolean isEmpty() { return pq.isEmpty(); }
    public void clear() { pq.clear(); }

    @Override
    public String toString() {
        return "BinaryHeap" + pq.toString();
    }
}

class RozeIndexMap {
    private final java.util.LinkedHashMap<Object, Object> map;
    private final java.util.ArrayList<Object> keyList;

    public RozeIndexMap() {
        this.map = new java.util.LinkedHashMap<>();
        this.keyList = new java.util.ArrayList<>();
    }

    public void insert(Object key, Object value) {
        if (!map.containsKey(key)) {
            keyList.add(key);
        }
        map.put(key, value);
    }

    public Object get(Object key) { return map.get(key); }
    public Object getIndexKey(int idx) {
        if (idx < 0 || idx >= keyList.size()) return null;
        return keyList.get(idx);
    }
    public Object getIndexValue(int idx) {
        if (idx < 0 || idx >= keyList.size()) return null;
        return map.get(keyList.get(idx));
    }
    public boolean containsKey(Object key) { return map.containsKey(key); }
    public Object remove(Object key) {
        if (map.containsKey(key)) {
            keyList.remove(key);
            return map.remove(key);
        }
        return null;
    }
    public long len() { return map.size(); }
    public boolean isEmpty() { return map.isEmpty(); }
    public java.util.List<Object> keys() { return new java.util.ArrayList<>(keyList); }
    public java.util.List<Object> values() {
        java.util.ArrayList<Object> vals = new java.util.ArrayList<>();
        for (Object k : keyList) vals.add(map.get(k));
        return vals;
    }
    public void clear() { map.clear(); keyList.clear(); }

    @Override
    public String toString() {
        return "IndexMap" + map.toString();
    }
}

class RozeRingBuffer {
    private final Object[] buffer;
    private final int capacity;
    private int head = 0;
    private int count = 0;

    public RozeRingBuffer(int capacity) {
        this.capacity = Math.max(capacity, 1);
        this.buffer = new Object[this.capacity];
    }

    public boolean push(Object val) {
        if (count == capacity) return false;
        buffer[(head + count) % capacity] = val;
        count++;
        return true;
    }

    public void pushOverwrite(Object val) {
        if (count == capacity) {
            buffer[head] = val;
            head = (head + 1) % capacity;
        } else {
            buffer[(head + count) % capacity] = val;
            count++;
        }
    }

    public Object pop() {
        if (count == 0) return null;
        Object val = buffer[head];
        buffer[head] = null;
        head = (head + 1) % capacity;
        count--;
        return val;
    }

    public Object peek() {
        if (count == 0) return null;
        return buffer[head];
    }

    public long len() { return count; }
    public long capacity() { return capacity; }
    public boolean isEmpty() { return count == 0; }
    public boolean isFull() { return count == capacity; }
    public void clear() {
        java.util.Arrays.fill(buffer, null);
        head = 0;
        count = 0;
    }

    @Override
    public String toString() {
        return "RingBuffer(cap=" + capacity + ", len=" + count + ")";
    }
}

class RozeBitSet {
    private final java.util.BitSet bs;

    public RozeBitSet() {
        this.bs = new java.util.BitSet();
    }

    public RozeBitSet(int nbits) {
        this.bs = new java.util.BitSet(Math.max(nbits, 1));
    }

    public void set(int idx) { if (idx >= 0) bs.set(idx); }
    public void clearBit(int idx) { if (idx >= 0) bs.clear(idx); }
    public boolean test(int idx) { return idx >= 0 && bs.get(idx); }
    public void flip(int idx) { if (idx >= 0) bs.flip(idx); }
    public long countOnes() { return bs.cardinality(); }
    public long len() { return Math.max(bs.length(), bs.size()); }
    public boolean isEmpty() { return bs.isEmpty(); }
    public void unionWith(RozeBitSet other) { bs.or(other.bs); }
    public void intersectWith(RozeBitSet other) { bs.and(other.bs); }
    public void differenceWith(RozeBitSet other) { bs.andNot(other.bs); }
    public void clear() { bs.clear(); }

    @Override
    public String toString() {
        return "BitSet" + bs.toString();
    }
}
"#;

const COLLECTIONS_HELPER_METHODS: &str = r#"
    // VecDeque
    private static RozeVecDeque vec_deque_new() { return new RozeVecDeque(); }
    private static RozeVecDeque VecDeque_new() { return new RozeVecDeque(); }
    private static RozeVecDeque vec_deque_with_capacity(long cap) { return new RozeVecDeque((int)cap); }
    private static void vec_deque_push_back(Object vd, Object val) { ((RozeVecDeque) vd).pushBack(val); }
    private static void vec_deque_push_front(Object vd, Object val) { ((RozeVecDeque) vd).pushFront(val); }
    private static Object vec_deque_pop_back(Object vd) { return ((RozeVecDeque) vd).popBack(); }
    private static Object vec_deque_pop_front(Object vd) { return ((RozeVecDeque) vd).popFront(); }
    private static Object vec_deque_front(Object vd) { return ((RozeVecDeque) vd).front(); }
    private static Object vec_deque_back(Object vd) { return ((RozeVecDeque) vd).back(); }
    private static Object vec_deque_get(Object vd, long idx) { return ((RozeVecDeque) vd).get((int)idx); }
    private static void vec_deque_set(Object vd, long idx, Object val) { ((RozeVecDeque) vd).set((int)idx, val); }
    private static long vec_deque_len(Object vd) { return ((RozeVecDeque) vd).len(); }
    private static boolean vec_deque_is_empty(Object vd) { return ((RozeVecDeque) vd).isEmpty(); }
    private static void vec_deque_clear(Object vd) { ((RozeVecDeque) vd).clear(); }

    // BTreeMap
    private static RozeBTreeMap btree_map_new() { return new RozeBTreeMap(); }
    private static RozeBTreeMap BTreeMap_new() { return new RozeBTreeMap(); }
    private static RozeBTreeMap btree_map_new_str() { return new RozeBTreeMap(); }
    private static void btree_map_insert(Object m, Object k, Object v) { ((RozeBTreeMap) m).insert(k, v); }
    private static Object btree_map_get(Object m, Object k) { return ((RozeBTreeMap) m).get(k); }
    private static boolean btree_map_contains_key(Object m, Object k) { return ((RozeBTreeMap) m).containsKey(k); }
    private static Object btree_map_remove(Object m, Object k) { return ((RozeBTreeMap) m).remove(k); }
    private static long btree_map_len(Object m) { return ((RozeBTreeMap) m).len(); }
    private static boolean btree_map_is_empty(Object m) { return ((RozeBTreeMap) m).isEmpty(); }
    private static Object btree_map_first_key(Object m) { return ((RozeBTreeMap) m).firstKey(); }
    private static Object btree_map_last_key(Object m) { return ((RozeBTreeMap) m).lastKey(); }
    private static java.util.List<Object> btree_map_keys(Object m) { return ((RozeBTreeMap) m).keys(); }
    private static java.util.List<Object> btree_map_values(Object m) { return ((RozeBTreeMap) m).values(); }
    private static void btree_map_clear(Object m) { ((RozeBTreeMap) m).clear(); }

    // BTreeSet
    private static RozeBTreeSet btree_set_new() { return new RozeBTreeSet(); }
    private static RozeBTreeSet BTreeSet_new() { return new RozeBTreeSet(); }
    private static RozeBTreeSet btree_set_new_str() { return new RozeBTreeSet(); }
    private static boolean btree_set_insert(Object s, Object val) { return ((RozeBTreeSet) s).insert(val); }
    private static boolean btree_set_contains(Object s, Object val) { return ((RozeBTreeSet) s).contains(val); }
    private static boolean btree_set_remove(Object s, Object val) { return ((RozeBTreeSet) s).remove(val); }
    private static long btree_set_len(Object s) { return ((RozeBTreeSet) s).len(); }
    private static boolean btree_set_is_empty(Object s) { return ((RozeBTreeSet) s).isEmpty(); }
    private static Object btree_set_first(Object s) { return ((RozeBTreeSet) s).first(); }
    private static Object btree_set_last(Object s) { return ((RozeBTreeSet) s).last(); }
    private static java.util.List<Object> btree_set_to_list(Object s) { return ((RozeBTreeSet) s).toList(); }
    private static void btree_set_clear(Object s) { ((RozeBTreeSet) s).clear(); }

    // BinaryHeap
    private static RozeBinaryHeap binary_heap_new() { return new RozeBinaryHeap(); }
    private static RozeBinaryHeap BinaryHeap_new() { return new RozeBinaryHeap(); }
    private static void binary_heap_push(Object h, Object val) { ((RozeBinaryHeap) h).push(val); }
    private static Object binary_heap_pop(Object h) { return ((RozeBinaryHeap) h).pop(); }
    private static Object binary_heap_peek(Object h) { return ((RozeBinaryHeap) h).peek(); }
    private static long binary_heap_len(Object h) { return ((RozeBinaryHeap) h).len(); }
    private static boolean binary_heap_is_empty(Object h) { return ((RozeBinaryHeap) h).isEmpty(); }
    private static void binary_heap_clear(Object h) { ((RozeBinaryHeap) h).clear(); }

    // IndexMap
    private static RozeIndexMap index_map_new() { return new RozeIndexMap(); }
    private static RozeIndexMap IndexMap_new() { return new RozeIndexMap(); }
    private static RozeIndexMap index_map_new_str() { return new RozeIndexMap(); }
    private static void index_map_insert(Object m, Object k, Object v) { ((RozeIndexMap) m).insert(k, v); }
    private static Object index_map_get(Object m, Object k) { return ((RozeIndexMap) m).get(k); }
    private static Object index_map_get_index_key(Object m, long idx) { return ((RozeIndexMap) m).getIndexKey((int)idx); }
    private static Object index_map_get_index_value(Object m, long idx) { return ((RozeIndexMap) m).getIndexValue((int)idx); }
    private static boolean index_map_contains_key(Object m, Object k) { return ((RozeIndexMap) m).containsKey(k); }
    private static Object index_map_remove(Object m, Object k) { return ((RozeIndexMap) m).remove(k); }
    private static long index_map_len(Object m) { return ((RozeIndexMap) m).len(); }
    private static boolean index_map_is_empty(Object m) { return ((RozeIndexMap) m).isEmpty(); }
    private static java.util.List<Object> index_map_keys(Object m) { return ((RozeIndexMap) m).keys(); }
    private static java.util.List<Object> index_map_values(Object m) { return ((RozeIndexMap) m).values(); }
    private static void index_map_clear(Object m) { ((RozeIndexMap) m).clear(); }

    // RingBuffer
    private static RozeRingBuffer ring_buffer_new(long cap) { return new RozeRingBuffer((int)cap); }
    private static RozeRingBuffer RingBuffer_new(long cap) { return new RozeRingBuffer((int)cap); }
    private static boolean ring_buffer_push(Object rb, Object val) { return ((RozeRingBuffer) rb).push(val); }
    private static void ring_buffer_push_overwrite(Object rb, Object val) { ((RozeRingBuffer) rb).pushOverwrite(val); }
    private static Object ring_buffer_pop(Object rb) { return ((RozeRingBuffer) rb).pop(); }
    private static Object ring_buffer_peek(Object rb) { return ((RozeRingBuffer) rb).peek(); }
    private static long ring_buffer_len(Object rb) { return ((RozeRingBuffer) rb).len(); }
    private static long ring_buffer_capacity(Object rb) { return ((RozeRingBuffer) rb).capacity(); }
    private static boolean ring_buffer_is_empty(Object rb) { return ((RozeRingBuffer) rb).isEmpty(); }
    private static boolean ring_buffer_is_full(Object rb) { return ((RozeRingBuffer) rb).isFull(); }
    private static void ring_buffer_clear(Object rb) { ((RozeRingBuffer) rb).clear(); }

    // BitSet
    private static RozeBitSet bit_set_new() { return new RozeBitSet(); }
    private static RozeBitSet BitSet_new() { return new RozeBitSet(); }
    private static RozeBitSet bit_set_with_capacity(long cap) { return new RozeBitSet((int)cap); }
    private static void bit_set_set(Object bs, long idx) { ((RozeBitSet) bs).set((int)idx); }
    private static void bit_set_clear_bit(Object bs, long idx) { ((RozeBitSet) bs).clearBit((int)idx); }
    private static boolean bit_set_test(Object bs, long idx) { return ((RozeBitSet) bs).test((int)idx); }
    private static void bit_set_flip(Object bs, long idx) { ((RozeBitSet) bs).flip((int)idx); }
    private static long bit_set_count_ones(Object bs) { return ((RozeBitSet) bs).countOnes(); }
    private static long bit_set_len(Object bs) { return ((RozeBitSet) bs).len(); }
    private static boolean bit_set_is_empty(Object bs) { return ((RozeBitSet) bs).isEmpty(); }
    private static void bit_set_union(Object bs, Object other) { ((RozeBitSet) bs).unionWith((RozeBitSet) other); }
    private static void bit_set_intersect(Object bs, Object other) { ((RozeBitSet) bs).intersectWith((RozeBitSet) other); }
    private static void bit_set_difference(Object bs, Object other) { ((RozeBitSet) bs).differenceWith((RozeBitSet) other); }
    private static void bit_set_clear(Object bs) { ((RozeBitSet) bs).clear(); }
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
        "substring" | "string_substring" => Some("String"),
        "string_contains" | "string_starts_with" | "string_ends_with" => Some("boolean"),
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
        "map_put" | "map_set" => Some(OBJECT_TYPE),
        "map_get" => Some(OBJECT_TYPE),
        "map_has" => Some("boolean"),
        "map_remove" => Some(OBJECT_TYPE),
        "map_size" => Some("int"),
        "map_is_empty" => Some("boolean"),
        "map_keys" => Some("java.util.List"),

        "read_file" => Some("String"),
        "write_file" => Some("void"),
        "append_file" => Some("void"),
        "file_exists" => Some("boolean"),
        "delete_file" => Some("boolean"),
        "read_lines" => Some("java.util.List"),

        "http_get" => Some("String"),
        "http_post" => Some("String"),
        "http_request" => Some("java.util.Map"),

        "json_encode" => Some("String"),
        "json_decode" => Some(OBJECT_TYPE),

        "http_server_start" => Some("java.net.ServerSocket"),
        "http_server_accept" => Some("java.util.Map"),
        "http_server_respond" => Some("void"),
        "http_server_stop" => Some("void"),
        "http_server_listen" => Some(OBJECT_TYPE),
        "http_server_respond_json" => Some("void"),
        "http_server_respond_headers" => Some("void"),
        "url_route_match" => Some("java.util.Map"),

        "sql_connect" => Some(OBJECT_TYPE),
        "sql_query" => Some("java.util.List"),
        "sql_execute" => Some("int"),
        "sql_close" => Some("void"),

        // Database Connection Pooling (ADV-03)
        "db_pool_create" | "db_pool_get" => Some(OBJECT_TYPE),
        "db_pool_release" | "db_pool_close" => Some("void"),
        "db_pool_stats" => Some("java.util.Map"),
        "db_pool_query" => Some("java.util.List"),
        "db_pool_execute" => Some("int"),

        // JWT Security (ADV-03)
        "jwt_sign" => Some("String"),
        "jwt_verify" => Some("boolean"),
        "jwt_decode" | "jwt_decode_header" => Some("java.util.Map"),

        // Time (ADV-03)
        "time_now" | "time_now_millis" => Some("int"),

        // Option
        "option_some" | "Some" => Some("RozeOption"),
        "option_none" | "None" => Some("RozeOption"),
        "option_is_some" | "is_some" => Some("boolean"),
        "option_is_none" | "is_none" => Some("boolean"),
        "option_unwrap" => Some(OBJECT_TYPE),
        "option_unwrap_or" => Some(OBJECT_TYPE),

        // Result
        "result_ok" | "Ok" => Some("RozeResult"),
        "result_err" | "Err" => Some("RozeResult"),
        "result_is_ok" | "is_ok" => Some("boolean"),
        "result_is_err" | "is_err" => Some("boolean"),
        "result_unwrap" | "unwrap" => Some(OBJECT_TYPE),
        "result_unwrap_err" | "unwrap_err" => Some(OBJECT_TYPE),
        "result_unwrap_or" | "unwrap_or" => Some(OBJECT_TYPE),

        // Safe IO
        "read_file_result" => Some("RozeResult"),
        "write_file_result" => Some("RozeResult"),
        "append_file_result" => Some("RozeResult"),
        "delete_file_result" => Some("RozeResult"),
        "read_lines_result" => Some("RozeResult"),
        "http_get_result" => Some("RozeResult"),
        "http_post_result" => Some("RozeResult"),
        "to_int_result" => Some("RozeResult"),

        // Safe collections
        "list_get_option" => Some("RozeOption"),
        "map_get_option" => Some("RozeOption"),

        // Concurrency: Threads & Channels (MED-03)
        "thread_spawn" => Some("RozeThreadHandle"),
        "thread_join" => Some(OBJECT_TYPE),
        "thread_sleep" => Some("void"),
        "thread_current_id" => Some("int"),
        "fiber_spawn" => Some("RozeFiberHandle"),
        "fiber_join" => Some(OBJECT_TYPE),
        "fiber_yield" => Some("void"),
        "fiber_sleep" => Some("void"),
        "fiber_id" => Some("int"),
        "fiber_count" => Some("int"),
        "fiber_stats" => Some("void"),
        "future_spawn" | "__roze_async_spawn" => Some("RozeFuture"),
        "future_await" | "__roze_future_await" => Some(OBJECT_TYPE),
        "future_poll" | "__roze_future_poll" => Some("boolean"),
        "future_ready" | "__roze_future_ready" => Some("RozeFuture"),
        "async_sleep" | "__roze_async_sleep" => Some("RozeFuture"),
        "channel_new" => Some("RozeChannel"),
        "channel_unbounded" => Some("RozeChannel"),
        "channel_send" | "send" => Some("boolean"),
        "channel_try_send" => Some("boolean"),
        "channel_recv" | "recv" => Some(OBJECT_TYPE),
        "channel_try_recv" => Some(OBJECT_TYPE),
        "channel_close" => Some("void"),
        "channel_is_closed" => Some("boolean"),
        "channel_len" => Some("int"),
        "channel_capacity" => Some("int"),

        // Mutex & RwLock (M1-T06)
        "mutex_new" => Some("RozeMutex"),
        "mutex_lock" | "mutex_unlock" => Some("void"),
        "mutex_try_lock" => Some("boolean"),

        "rwlock_new" => Some("RozeRwLock"),
        "rwlock_read_lock" | "rwlock_read_unlock" | "rwlock_write_lock" | "rwlock_write_unlock" => Some("void"),
        "rwlock_try_read_lock" | "rwlock_try_write_lock" => Some("boolean"),

        // Atomics (M1-T06)
        "atomic_bool_new" => Some("java.util.concurrent.atomic.AtomicBoolean"),
        "atomic_bool_load" | "atomic_bool_swap" | "atomic_bool_cas" => Some("boolean"),
        "atomic_bool_store" => Some("void"),

        "atomic_i64_new" => Some("java.util.concurrent.atomic.AtomicLong"),
        "atomic_i64_load" | "atomic_i64_swap" | "atomic_i64_add" | "atomic_i64_sub" | "atomic_i64_and" | "atomic_i64_or" | "atomic_i64_xor" => Some("int"),
        "atomic_i64_store" => Some("void"),
        "atomic_i64_cas" => Some("boolean"),

        "atomic_usize_new" => Some("java.util.concurrent.atomic.AtomicLong"),
        "atomic_usize_load" | "atomic_usize_swap" | "atomic_usize_add" | "atomic_usize_sub" => Some("int"),
        "atomic_usize_store" => Some("void"),
        "atomic_usize_cas" => Some("boolean"),

        "atomic_ptr_new" => Some("java.util.concurrent.atomic.AtomicReference"),
        "atomic_ptr_load" | "atomic_ptr_swap" => Some(OBJECT_TYPE),
        "atomic_ptr_store" => Some("void"),
        "atomic_ptr_cas" => Some("boolean"),

        "atomic_load" | "atomic_swap" => Some(OBJECT_TYPE),
        "atomic_store" => Some("void"),
        "atomic_cas" => Some("boolean"),
        "atomic_add" | "atomic_sub" => Some("int"),

        // Structured Concurrency: Nursery, Task Groups & Cancelation (M1-T07)
        "nursery_new" | "task_group_new" |
        "nursery_on_failure" | "task_group_on_failure" |
        "nursery_on_success" | "task_group_on_success" |
        "nursery_with_timeout" | "task_group_with_timeout" => Some("RozeTaskGroup"),

        "nursery_fork" | "task_group_fork" |
        "nursery_spawn" | "task_group_spawn" => Some("RozeSubtask"),

        "nursery_join" | "task_group_join" |
        "nursery_wait" | "task_group_wait" |
        "nursery_result" | "task_group_result" => Some("int"),

        "nursery_cancel" | "task_group_cancel" => Some("void"),
        "nursery_is_cancelled" | "task_group_is_cancelled" => Some("boolean"),

        "nursery_token" | "task_group_token" => Some("RozeCancellationToken"),

        "nursery_task_count" | "task_group_task_count" |
        "nursery_completed_count" | "task_group_completed_count" => Some("int"),

        "subtask_get" => Some(OBJECT_TYPE),
        "subtask_is_done" | "subtask_is_cancelled" => Some("boolean"),

        "cancel_token_new" => Some("RozeCancellationToken"),
        "cancel_token_cancel" => Some("void"),
        "cancel_token_is_cancelled" => Some("boolean"),

        // Unsafe Raw Pointers & Direct Memory (MED-05)
        "alloc" | "alloc_zeroed" | "realloc" => Some("long"),
        "free" => Some("void"),
        "ptr_read_u8" | "ptr_read" | "ptr_read_i64" | "ptr_read_at" | "ptr_read_u8_at" => Some("int"),
        "ptr_write_u8" | "ptr_write" | "ptr_write_i64" | "ptr_write_at" | "ptr_write_u8_at" => Some("void"),
        "ptr_add" | "ptr_offset" => Some("long"),
        "ptr_diff" | "ptr_to_int" => Some("int"),
        "int_to_ptr" => Some("long"),
        "ptr_is_null" => Some("boolean"),
        "mem_copy" | "mem_set" => Some("void"),

        // Freestanding VGA Text Driver (ADV-01)
        "vga_clear" | "vga_print" | "vga_write_char" | "vga_write_byte" | "vga_set_cursor" | "vga_set_color" => Some("void"),

        // Freestanding Kernel & Multiboot2 (ADV-02)
        "multiboot2_get_magic" | "inb" => Some("int"),
        "multiboot2_get_info_ptr" => Some("long"),
        "outb" | "qemu_exit" | "serial_print" | "serial_putchar" => Some("void"),

        // Fixed-Width Numeric Suite (M2-T01)
        "i8" => Some("byte"),
        "u8" => Some("int"),
        "i16" => Some("short"),
        "u16" => Some("int"),
        "i32" => Some("int"),
        "u32" => Some("long"),
        "i64" => Some("long"),
        "u64" => Some("long"),
        "i128" => Some("java.math.BigInteger"),
        "u128" => Some("java.math.BigInteger"),
        "isize" => Some("long"),
        "usize" => Some("long"),
        "f32" => Some("float"),
        "f64" => Some("double"),
        "char" => Some("char"),
        "int" => Some("int"),
        "float" => Some("double"),

        "i8_min" | "i8_max" => Some("byte"),
        "u8_min" | "u8_max" => Some("int"),
        "i16_min" | "i16_max" => Some("short"),
        "u16_min" | "u16_max" => Some("int"),
        "i32_min" | "i32_max" => Some("int"),
        "u32_min" | "u32_max" => Some("long"),
        "i64_min" | "i64_max" => Some("long"),
        "u64_min" | "u64_max" => Some("long"),
        "i128_min" | "i128_max" => Some("java.math.BigInteger"),
        "u128_min" | "u128_max" => Some("java.math.BigInteger"),
        "isize_min" | "isize_max" => Some("long"),
        "usize_min" | "usize_max" => Some("long"),
        "f32_min" | "f32_max" => Some("float"),
        "f64_min" | "f64_max" => Some("double"),

        // Hardware SIMD Vector Types (M2-T02)
        "Vec4f" | "vec4f" | "Vec4f_splat" | "vec4f_splat" | "Vec4f_zero" | "vec4f_zero" | "vec4f_add" | "vec4f_sub" | "vec4f_mul" | "vec4f_div" | "vec4f_min" | "vec4f_max" | "vec4f_sqrt" | "vec4f_abs" | "vec4f_fma" | "vec4f_set" | "vec4f_load" => Some("RozeVec4f"),
        "vec4f_dot" | "vec4f_sum" | "vec4f_get" => Some("float"),
        "vec4f_store" => Some("void"),

        "Vec8f" | "vec8f" | "Vec8f_splat" | "vec8f_splat" | "Vec8f_zero" | "vec8f_zero" | "vec8f_add" | "vec8f_sub" | "vec8f_mul" | "vec8f_div" | "vec8f_min" | "vec8f_max" | "vec8f_sqrt" | "vec8f_abs" | "vec8f_fma" | "vec8f_set" | "vec8f_load" => Some("RozeVec8f"),
        "vec8f_dot" | "vec8f_sum" | "vec8f_get" => Some("float"),
        "vec8f_store" => Some("void"),

        "Vec4i" | "vec4i" | "Vec4i_splat" | "vec4i_splat" | "Vec4i_zero" | "vec4i_zero" | "vec4i_add" | "vec4i_sub" | "vec4i_mul" | "vec4i_div" | "vec4i_min" | "vec4i_max" | "vec4i_abs" | "vec4i_set" | "vec4i_load" => Some("RozeVec4i"),
        "vec4i_sum" | "vec4i_get" => Some("int"),
        "vec4i_store" => Some("void"),

        "Vec16u8" | "vec16u8" | "Vec16u8_splat" | "vec16u8_splat" | "Vec16u8_zero" | "vec16u8_zero" | "vec16u8_add" | "vec16u8_sub" | "vec16u8_mul" | "vec16u8_saturating_add" | "vec16u8_saturating_sub" | "vec16u8_min" | "vec16u8_max" | "vec16u8_set" | "vec16u8_load" => Some("RozeVec16u8"),
        "vec16u8_sum" => Some("long"),
        "vec16u8_get" => Some("int"),
        "vec16u8_store" => Some("void"),

        // Advanced Standard Collections (M2-T03)
        // VecDeque
        "vec_deque_new" | "VecDeque_new" | "vec_deque_with_capacity" => Some("RozeVecDeque"),
        "vec_deque_push_back" | "vec_deque_push_front" | "vec_deque_set" | "vec_deque_clear" => Some("void"),
        "vec_deque_pop_back" | "vec_deque_pop_front" | "vec_deque_front" | "vec_deque_back" | "vec_deque_get" => Some(OBJECT_TYPE),
        "vec_deque_len" => Some("long"),
        "vec_deque_is_empty" => Some("boolean"),

        // BTreeMap
        "btree_map_new" | "BTreeMap_new" | "btree_map_new_str" => Some("RozeBTreeMap"),
        "btree_map_insert" | "btree_map_clear" => Some("void"),
        "btree_map_get" | "btree_map_remove" | "btree_map_first_key" | "btree_map_last_key" => Some(OBJECT_TYPE),
        "btree_map_contains_key" | "btree_map_is_empty" => Some("boolean"),
        "btree_map_len" => Some("long"),
        "btree_map_keys" | "btree_map_values" => Some("java.util.List"),

        // BTreeSet
        "btree_set_new" | "BTreeSet_new" | "btree_set_new_str" => Some("RozeBTreeSet"),
        "btree_set_insert" | "btree_set_contains" | "btree_set_remove" | "btree_set_is_empty" => Some("boolean"),
        "btree_set_len" => Some("long"),
        "btree_set_first" | "btree_set_last" => Some(OBJECT_TYPE),
        "btree_set_to_list" => Some("java.util.List"),
        "btree_set_clear" => Some("void"),

        // BinaryHeap
        "binary_heap_new" | "BinaryHeap_new" => Some("RozeBinaryHeap"),
        "binary_heap_push" | "binary_heap_clear" => Some("void"),
        "binary_heap_pop" | "binary_heap_peek" => Some(OBJECT_TYPE),
        "binary_heap_len" => Some("long"),
        "binary_heap_is_empty" => Some("boolean"),

        // IndexMap
        "index_map_new" | "IndexMap_new" | "index_map_new_str" => Some("RozeIndexMap"),
        "index_map_insert" | "index_map_clear" => Some("void"),
        "index_map_get" | "index_map_get_index_key" | "index_map_get_index_value" | "index_map_remove" => Some(OBJECT_TYPE),
        "index_map_contains_key" | "index_map_is_empty" => Some("boolean"),
        "index_map_len" => Some("long"),
        "index_map_keys" | "index_map_values" => Some("java.util.List"),

        // RingBuffer
        "ring_buffer_new" | "RingBuffer_new" => Some("RozeRingBuffer"),
        "ring_buffer_push" | "ring_buffer_is_empty" | "ring_buffer_is_full" => Some("boolean"),
        "ring_buffer_push_overwrite" | "ring_buffer_clear" => Some("void"),
        "ring_buffer_pop" | "ring_buffer_peek" => Some(OBJECT_TYPE),
        "ring_buffer_len" | "ring_buffer_capacity" => Some("long"),

        // BitSet
        "bit_set_new" | "BitSet_new" | "bit_set_with_capacity" => Some("RozeBitSet"),
        "bit_set_set" | "bit_set_clear_bit" | "bit_set_flip" | "bit_set_union" | "bit_set_intersect" | "bit_set_difference" | "bit_set_clear" => Some("void"),
        "bit_set_test" | "bit_set_is_empty" => Some("boolean"),
        "bit_set_count_ones" | "bit_set_len" => Some("long"),

        _ => None,
    }
}

pub(crate) fn is_intrinsic(name: &str) -> bool {
    intrinsic_return_type(name).is_some()
}

/// Java's true reserved words (keywords + reserved literals) -- these
/// can never be used as a Java identifier, unlike Java's *contextual*
/// keywords (`var`, `yield`, `record`, `sealed`, `permits`), which
/// remain legal identifiers in most positions and so aren't escaped
/// here (escaping them would just be unnecessary noise in the common
/// case where they're never actually used as a Roze name).
const JAVA_RESERVED_WORDS: &[&str] = &[
    "abstract", "assert", "boolean", "break", "byte", "case", "catch",
    "char", "class", "const", "continue", "default", "do", "double",
    "else", "enum", "extends", "final", "finally", "float", "for",
    "goto", "if", "implements", "import", "instanceof", "int",
    "interface", "long", "native", "new", "package", "private",
    "protected", "public", "return", "short", "static", "strictfp",
    "super", "switch", "synchronized", "this", "throw", "throws",
    "transient", "try", "void", "volatile", "while",
    "true", "false", "null",
    "_",
];

/// Escapes a Roze identifier that happens to collide with a Java
/// reserved word (e.g. a Roze function or variable named `assert`,
/// `interface`, `synchronized`...) by appending an underscore, so
/// codegen never emits an invalid Java identifier. Must be applied
/// consistently at *every* definition and use site (every caller of
/// this function is one) -- missing even one would silently produce a
/// mismatch between the mangled name at the declaration and the
/// original name at a use site, which javac would reject as an
/// undefined symbol.
fn escape_java_identifier(name: &str) -> String {
    if name == "this" {
        return "this".to_string();
    }
    if JAVA_RESERVED_WORDS.contains(&name) {
        format!("{}_", name)
    } else {
        name.to_string()
    }
}

/// Like `ty.to_java()`, but additionally escapes a `Type::Class` name
/// against Java's reserved words -- `Type::to_java()` itself can't do
/// this (it lives in `semantic`, which reserved-word escaping has no
/// business depending on; that's a codegen-specific concern). Every
/// site that emits a *declared type* as Java source text (a variable's
/// declared type, a parameter, a return type, a field) needs this
/// instead of calling `.to_java()` directly, or a class named e.g.
/// `interface` would correctly escape at its `new interface_(...)`
/// call site but not in the type position right next to it.
fn java_type_name(ty: &Type) -> String {
    match ty {
        Type::Class(name) | Type::Trait(name) => escape_java_identifier(name),
        Type::Generic { base, type_args } => {
            if base == "List" || base == "list" {
                "java.util.List".to_string()
            } else if base == "Map" || base == "map" {
                "java.util.Map".to_string()
            } else if base == "Option" || base == "option" {
                "RozeOption".to_string()
            } else if base == "Result" || base == "result" {
                "RozeResult".to_string()
            } else if base == "Channel" || base == "channel" {
                "RozeChannel".to_string()
            } else if base == "Thread" || base == "thread" {
                "RozeThreadHandle".to_string()
            } else if base == "Fiber" || base == "fiber" {
                "RozeFiberHandle".to_string()
            } else if base == "Future" || base == "future" {
                "RozeFuture".to_string()
            } else if base == "Mutex" || base == "mutex" {
                "RozeMutex".to_string()
            } else if base == "RwLock" || base == "rwlock" {
                "RozeRwLock".to_string()
            } else if base == "AtomicBool" || base == "atomic_bool" {
                "java.util.concurrent.atomic.AtomicBoolean".to_string()
            } else if base == "AtomicI64" || base == "atomic_i64" || base == "AtomicUsize" || base == "atomic_usize" {
                "java.util.concurrent.atomic.AtomicLong".to_string()
            } else if base == "AtomicPtr" || base == "atomic_ptr" {
                "java.util.concurrent.atomic.AtomicReference".to_string()
            } else if base == "TaskGroup" || base == "task_group" || base == "Nursery" || base == "nursery" {
                "RozeTaskGroup".to_string()
            } else if base == "Subtask" || base == "subtask" {
                "RozeSubtask".to_string()
            } else if base == "CancellationToken" || base == "cancellation_token" || base == "CancelToken" || base == "cancel_token" {
                "RozeCancellationToken".to_string()
            } else if base == "VecDeque" || base == "vec_deque" {
                "RozeVecDeque".to_string()
            } else if base == "BTreeMap" || base == "btree_map" {
                "RozeBTreeMap".to_string()
            } else if base == "BTreeSet" || base == "btree_set" {
                "RozeBTreeSet".to_string()
            } else if base == "BinaryHeap" || base == "binary_heap" {
                "RozeBinaryHeap".to_string()
            } else if base == "IndexMap" || base == "index_map" {
                "RozeIndexMap".to_string()
            } else if base == "RingBuffer" || base == "ring_buffer" {
                "RozeRingBuffer".to_string()
            } else if base == "BitSet" || base == "bit_set" {
                "RozeBitSet".to_string()
            } else {
                escape_java_identifier(&crate::semantic::specialized_name(base, type_args))
            }
        }
        other => other.to_java(),
    }
}

fn java_unbox(ty: &Type, expr_str: &str) -> String {
    match ty {
        Type::Int => format!("((Number) {}).intValue()", expr_str),
        Type::I8 => format!("((Number) {}).byteValue()", expr_str),
        Type::U8 => format!("(((Number) {}).intValue() & 0xFF)", expr_str),
        Type::I16 => format!("((Number) {}).shortValue()", expr_str),
        Type::U16 => format!("(((Number) {}).intValue() & 0xFFFF)", expr_str),
        Type::I32 => format!("((Number) {}).intValue()", expr_str),
        Type::U32 => format!("(((Number) {}).longValue() & 0xFFFFFFFFL)", expr_str),
        Type::I64 | Type::U64 | Type::Isize | Type::Usize => format!("((Number) {}).longValue()", expr_str),
        Type::I128 => format!("__roze_to_i128({})", expr_str),
        Type::U128 => format!("__roze_to_u128({})", expr_str),
        Type::Float => format!("((Number) {}).doubleValue()", expr_str),
        Type::F32 => format!("((Number) {}).floatValue()", expr_str),
        Type::F64 => format!("((Number) {}).doubleValue()", expr_str),
        Type::Char => format!("((Character) {})", expr_str),
        Type::Bool => format!("((Boolean) {})", expr_str),
        Type::String => format!("((String) {})", expr_str),
        Type::Class(name) | Type::Trait(name) => format!("(({}) {})", escape_java_identifier(name), expr_str),
        Type::Enum(name) => format!("(({}) {})", escape_java_identifier(name), expr_str),
        Type::Generic { base, type_args } => {
            if base == "List" || base == "list" {
                format!("((java.util.List) {})", expr_str)
            } else if base == "Map" || base == "map" {
                format!("((java.util.Map) {})", expr_str)
            } else if base == "Option" || base == "option" {
                format!("((RozeOption) {})", expr_str)
            } else if base == "Result" || base == "result" {
                format!("((RozeResult) {})", expr_str)
            } else if base == "Channel" || base == "channel" {
                format!("((RozeChannel) {})", expr_str)
            } else if base == "Thread" || base == "thread" {
                format!("((RozeThreadHandle) {})", expr_str)
            } else if base == "Fiber" || base == "fiber" {
                format!("((RozeFiberHandle) {})", expr_str)
            } else if base == "Future" || base == "future" {
                format!("((RozeFuture) {})", expr_str)
            } else if base == "Mutex" || base == "mutex" {
                format!("((RozeMutex) {})", expr_str)
            } else if base == "RwLock" || base == "rwlock" {
                format!("((RozeRwLock) {})", expr_str)
            } else if base == "AtomicBool" || base == "atomic_bool" {
                format!("((java.util.concurrent.atomic.AtomicBoolean) {})", expr_str)
            } else if base == "AtomicI64" || base == "atomic_i64" || base == "AtomicUsize" || base == "atomic_usize" {
                format!("((java.util.concurrent.atomic.AtomicLong) {})", expr_str)
            } else if base == "AtomicPtr" || base == "atomic_ptr" {
                format!("((java.util.concurrent.atomic.AtomicReference) {})", expr_str)
            } else if base == "TaskGroup" || base == "task_group" || base == "Nursery" || base == "nursery" {
                format!("((RozeTaskGroup) {})", expr_str)
            } else if base == "Subtask" || base == "subtask" {
                format!("((RozeSubtask) {})", expr_str)
            } else if base == "CancellationToken" || base == "cancellation_token" || base == "CancelToken" || base == "cancel_token" {
                format!("((RozeCancellationToken) {})", expr_str)
            } else {
                let spec = crate::semantic::specialized_name(base, type_args);
                format!("(({}) {})", escape_java_identifier(&spec), expr_str)
            }
        }
        Type::Function { .. } => format!("((RozeFn) {})", expr_str),
        Type::List => format!("((java.util.List) {})", expr_str),
        Type::Map => format!("((java.util.Map) {})", expr_str),
        Type::Option => format!("((RozeOption) {})", expr_str),
        Type::Result => format!("((RozeResult) {})", expr_str),
        _ => expr_str.to_string(),
    }
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

pub struct JavaSourceGenerator {
    program: TypedProgram,
    class_name: String,
    try_counter: std::cell::Cell<usize>,
}

impl JavaSourceGenerator {
    /// Takes the fully type-annotated IR (see `crate::ir` and
    /// `semantic::check_and_lower`) rather than the raw AST. Every
    /// expression in it already carries its resolved `Type`, computed
    /// once by the type checker -- this generator never re-derives a
    /// type from scratch (no local scope-tracking, no `infer_type`),
    /// it just reads `.type_` off whatever node it's rendering.
    pub fn new(program: TypedProgram, class_name: String) -> Self {
        Self {
            program,
            class_name,
            try_counter: std::cell::Cell::new(0),
        }
    }

    pub fn generate(&self) -> Result<String> {
        let mut source = String::new();
        source.push_str("// Generated by Roze compiler v0.1\n");
        source.push_str("@FunctionalInterface\n");
        source.push_str("interface RozeFn {\n");
        source.push_str("    Object apply(Object... args);\n");
        source.push_str("}\n\n");
        source.push_str(&format!("public class {} {{\n", self.class_name));

        let mut main_found = false;
        for stmt in &self.program.statements {
            if let TypedStatement::Function { name, params, return_type, body, .. } = stmt {
                // Core intrinsics are handled entirely at each call site
                // (see generate_call) and never emitted as a real method,
                // even if a same-named function happens to be declared.
                if is_intrinsic(name) {
                    continue;
                }

                if name == "main" {
                    source.push_str("    public static void main(String[] args) {\n");
                    self.generate_statement(&mut source, body, 2, &Type::Void, false)?;
                    source.push_str("    }\n");
                    main_found = true;
                } else {
                    let params_sig: Vec<String> = params.iter()
                        .map(|p| format!("{} {}", java_type_name(&p.type_), escape_java_identifier(&p.name)))
                        .collect();

                    source.push_str(&format!(
                        "    public static {} {}({}) {{\n",
                        java_type_name(return_type), escape_java_identifier(name), params_sig.join(", ")
                    ));
                    self.generate_statement(&mut source, body, 2, return_type, false)?;
                    if return_type != &Type::Void && !Self::statement_always_returns(body) {
                        source.push_str(&format!("        throw new RuntimeException(\"Missing return statement in function '{}'\");\n", name));
                    }
                    source.push_str("    }\n");
                }
            }
            if let TypedStatement::ExternFunction { name, params, return_type, .. } = stmt {
                let params_sig: Vec<String> = params.iter()
                    .map(|p| format!("{} {}", java_type_name(&p.type_), escape_java_identifier(&p.name)))
                    .collect();
                source.push_str(&format!(
                    "    public static {} {}({}) {{\n",
                    java_type_name(return_type), escape_java_identifier(name), params_sig.join(", ")
                ));
                source.push_str(&format!(
                    "        throw new UnsupportedOperationException(\"extern C function '{}' is native-only; run with --target native\");\n",
                    name
                ));
                source.push_str("    }\n");
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
        source.push_str(ROZEBOOT_HELPER_METHODS);
        source.push_str(OPTION_RESULT_HELPER_METHODS);
        source.push_str(CONCURRENCY_HELPER_METHODS);
        source.push_str(DIRECT_MEMORY_HELPER_METHODS);
        source.push_str(NUMERIC_HELPER_METHODS);
        source.push_str(SIMD_HELPER_METHODS);
        source.push_str(COLLECTIONS_HELPER_METHODS);
        source.push_str("}\n");

        source.push_str(OPTION_RESULT_HELPER_CLASSES);
        source.push_str(CONCURRENCY_HELPER_CLASSES);
        source.push_str(SIMD_HELPER_CLASSES);
        source.push_str(COLLECTIONS_HELPER_CLASSES);

        for stmt in &self.program.statements {
            match stmt {
                TypedStatement::TraitDecl { name, methods, .. } => {
                    self.generate_trait(&mut source, name, methods)?;
                }
                TypedStatement::ClassDecl { name, fields, .. } => {
                    self.generate_class(&mut source, name, fields)?;
                }
                TypedStatement::EnumDecl { name, variants, .. } => {
                    self.generate_enum(&mut source, name, variants)?;
                }
                _ => {}
            }
        }

        Ok(source)
    }

    fn generate_enum(&self, source: &mut String, name: &str, variants: &[(String, Vec<Type>)]) -> Result<()> {
        let escaped_name = escape_java_identifier(name);
        source.push_str(&format!("abstract class {} {{\n", escaped_name));
        source.push_str("    public abstract int __tag();\n\n");

        for (tag, (vname, payload_types)) in variants.iter().enumerate() {
            let escaped_vname = escape_java_identifier(vname);
            source.push_str(&format!("    public static class {} extends {} {{\n", escaped_vname, escaped_name));

            for (i, ptype) in payload_types.iter().enumerate() {
                source.push_str(&format!("        public {} _{};\n", java_type_name(ptype), i));
            }

            let ctor_params: Vec<String> = payload_types.iter().enumerate()
                .map(|(i, ptype)| format!("{} _{}", java_type_name(ptype), i))
                .collect();
            source.push_str(&format!("        public {}({}) {{\n", escaped_vname, ctor_params.join(", ")));
            for i in 0..payload_types.len() {
                source.push_str(&format!("            this._{} = _{};\n", i, i));
            }
            source.push_str("        }\n");

            source.push_str(&format!("        @Override public int __tag() {{ return {}; }}\n", tag));

            if payload_types.is_empty() {
                source.push_str(&format!("        @Override public String toString() {{ return \"{}::{}\"; }}\n", name, vname));
            } else {
                let parts: Vec<String> = (0..payload_types.len()).map(|i| format!("_{}", i)).collect();
                source.push_str(&format!(
                    "        @Override public String toString() {{ return \"{}::{}(\" + {} + \")\"; }}\n",
                    name, vname, parts.join(" + \", \" + ")
                ));
            }

            source.push_str("        @Override public boolean equals(Object o) {\n");
            source.push_str("            if (this == o) return true;\n");
            source.push_str(&format!("            if (!(o instanceof {})) return false;\n", escaped_vname));
            if payload_types.is_empty() {
                source.push_str("            return true;\n");
            } else {
                source.push_str(&format!("            {} other = ({}) o;\n", escaped_vname, escaped_vname));
                let mut conds = Vec::new();
                for (i, ptype) in payload_types.iter().enumerate() {
                    match ptype {
                        Type::Int | Type::Bool => conds.push(format!("this._{} == other._{}", i, i)),
                        Type::Float => conds.push(format!("Double.compare(this._{}, other._{}) == 0", i, i)),
                        _ => conds.push(format!("java.util.Objects.equals(this._{}, other._{})", i, i)),
                    }
                }
                source.push_str(&format!("            return {};\n", conds.join(" && ")));
            }
            source.push_str("        }\n");

            let hash_fields: Vec<String> = (0..payload_types.len()).map(|i| format!("_{}", i)).collect();
            if hash_fields.is_empty() {
                source.push_str(&format!("        @Override public int hashCode() {{ return {}; }}\n", tag));
            } else {
                source.push_str(&format!(
                    "        @Override public int hashCode() {{ return java.util.Objects.hash({}, {}); }}\n",
                    tag, hash_fields.join(", ")
                ));
            }

            source.push_str("    }\n\n");
        }

        source.push_str("}\n\n");
        Ok(())
    }

    /// Emits a Roze `class` as a real, separate Java class in the same
    /// file -- fields only, a single all-fields constructor (arguments
    /// positional, in declaration order, matching `new Name(...)`'s own
    /// order), no methods/inheritance (see ROADMAP.md). Legal Java
    /// allows any number of classes per file as long as at most one is
    /// `public` and it matches the filename -- that's always the main
    /// class here, so every class generated from a Roze `class`
    /// declaration is deliberately package-private.
    fn generate_trait(&self, source: &mut String, name: &str, methods: &[crate::ir::TypedTraitMethod]) -> Result<()> {
        let escaped_name = escape_java_identifier(name);
        source.push_str(&format!("interface {} {{\n", escaped_name));
        for m in methods {
            let params_sig: Vec<String> = m.params.iter()
                .map(|p| format!("{} {}", java_type_name(&p.type_), escape_java_identifier(&p.name)))
                .collect();
            source.push_str(&format!(
                "    {} {}({});\n",
                java_type_name(&m.return_type), escape_java_identifier(&m.name), params_sig.join(", ")
            ));
        }
        source.push_str("}\n\n");
        Ok(())
    }

    fn generate_class(&self, source: &mut String, name: &str, fields: &[(String, Type)]) -> Result<()> {
        let escaped_name = escape_java_identifier(name);
        let mut implements_traits: Vec<String> = Vec::new();
        let mut class_methods: Vec<&TypedStatement> = Vec::new();
        for stmt in &self.program.statements {
            if let TypedStatement::ImplDecl { trait_name, class_name, methods, .. } = stmt {
                if class_name == name {
                    if let Some(t) = trait_name {
                        implements_traits.push(escape_java_identifier(t));
                    }
                    for m in methods {
                        class_methods.push(m);
                    }
                }
            }
        }

        let impl_clause = if implements_traits.is_empty() {
            String::new()
        } else {
            format!(" implements {}", implements_traits.join(", "))
        };

        source.push_str(&format!("class {}{} {{\n", escaped_name, impl_clause));

        for (field_name, field_type) in fields {
            source.push_str(&format!(
                "    public {} {};\n",
                java_type_name(field_type), escape_java_identifier(field_name)
            ));
        }

        let ctor_params: Vec<String> = fields.iter()
            .map(|(field_name, field_type)| format!("{} {}", java_type_name(field_type), escape_java_identifier(field_name)))
            .collect();
        source.push_str(&format!("    public {}({}) {{\n", escaped_name, ctor_params.join(", ")));
        for (field_name, _) in fields {
            let escaped_field = escape_java_identifier(field_name);
            source.push_str(&format!("        this.{} = {};\n", escaped_field, escaped_field));
        }
        source.push_str("    }\n");

        for method_stmt in class_methods {
            if let TypedStatement::Function { name: m_name, params, return_type, body, .. } = method_stmt {
                let params_sig: Vec<String> = params.iter()
                    .map(|p| format!("{} {}", java_type_name(&p.type_), escape_java_identifier(&p.name)))
                    .collect();
                source.push_str(&format!(
                    "    public {} {}({}) {{\n",
                    java_type_name(return_type), escape_java_identifier(m_name), params_sig.join(", ")
                ));
                self.generate_statement(source, body, 2, return_type, false)?;
                if return_type != &Type::Void && !Self::statement_always_returns(body) {
                    source.push_str(&format!("        throw new RuntimeException(\"Missing return statement in method '{}'\");\n", m_name));
                }
                source.push_str("    }\n");
            }
        }

        source.push_str("}\n\n");
        Ok(())
    }

    fn hoist_try_expr(
        &self,
        expr: &TypedExpression,
        hoisted_stmts: &mut String,
        indent_str: &str,
        return_type: &Type,
    ) -> Result<TypedExpression> {
        match &expr.kind {
            TypedExpressionKind::Try(inner) => {
                let hoisted_inner = self.hoist_try_expr(inner, hoisted_stmts, indent_str, return_type)?;
                let c = self.try_counter.get();
                self.try_counter.set(c + 2);
                let temp_var = format!("__try_{}", c);
                let unwrapped_var = format!("__unwrapped_{}", c + 1);

                hoisted_stmts.push_str(indent_str);
                hoisted_stmts.push_str(&format!("Object {} = ", temp_var));
                self.generate_expression(hoisted_stmts, &hoisted_inner)?;
                hoisted_stmts.push_str(";\n");

                let ret_expr = match return_type {
                    Type::Result => format!("(RozeResult) {}", temp_var),
                    Type::Option => format!("(RozeOption) {}", temp_var),
                    Type::Void => String::new(),
                    _ => temp_var.clone(),
                };

                hoisted_stmts.push_str(indent_str);
                if return_type == &Type::Void {
                    hoisted_stmts.push_str(&format!("if (__roze_is_err_or_none({})) return;\n", temp_var));
                } else {
                    hoisted_stmts.push_str(&format!("if (__roze_is_err_or_none({})) return {};\n", temp_var, ret_expr));
                }

                hoisted_stmts.push_str(indent_str);
                hoisted_stmts.push_str(&format!("Object {} = __roze_unwrap_try({});\n", unwrapped_var, temp_var));

                Ok(TypedExpression {
                    kind: TypedExpressionKind::Identifier(unwrapped_var),
                    type_: expr.type_.clone(),
                    location: expr.location.clone(),
                })
            }
            TypedExpressionKind::Binary { left, operator, right } => {
                let left_h = self.hoist_try_expr(left, hoisted_stmts, indent_str, return_type)?;
                let right_h = self.hoist_try_expr(right, hoisted_stmts, indent_str, return_type)?;
                Ok(TypedExpression {
                    kind: TypedExpressionKind::Binary {
                        left: Box::new(left_h),
                        operator: operator.clone(),
                        right: Box::new(right_h),
                    },
                    type_: expr.type_.clone(),
                    location: expr.location.clone(),
                })
            }
            TypedExpressionKind::Unary { operator, operand } => {
                let op_h = self.hoist_try_expr(operand, hoisted_stmts, indent_str, return_type)?;
                Ok(TypedExpression {
                    kind: TypedExpressionKind::Unary {
                        operator: operator.clone(),
                        operand: Box::new(op_h),
                    },
                    type_: expr.type_.clone(),
                    location: expr.location.clone(),
                })
            }
            TypedExpressionKind::Call { function, arguments } => {
                let mut args_h = Vec::with_capacity(arguments.len());
                for arg in arguments {
                    args_h.push(self.hoist_try_expr(arg, hoisted_stmts, indent_str, return_type)?);
                }
                Ok(TypedExpression {
                    kind: TypedExpressionKind::Call {
                        function: function.clone(),
                        arguments: args_h,
                    },
                    type_: expr.type_.clone(),
                    location: expr.location.clone(),
                })
            }
            TypedExpressionKind::New { class_name, arguments } => {
                let mut args_h = Vec::with_capacity(arguments.len());
                for arg in arguments {
                    args_h.push(self.hoist_try_expr(arg, hoisted_stmts, indent_str, return_type)?);
                }
                Ok(TypedExpression {
                    kind: TypedExpressionKind::New {
                        class_name: class_name.clone(),
                        arguments: args_h,
                    },
                    type_: expr.type_.clone(),
                    location: expr.location.clone(),
                })
            }
            TypedExpressionKind::FieldAccess { object, field } => {
                let obj_h = self.hoist_try_expr(object, hoisted_stmts, indent_str, return_type)?;
                Ok(TypedExpression {
                    kind: TypedExpressionKind::FieldAccess {
                        object: Box::new(obj_h),
                        field: field.clone(),
                    },
                    type_: expr.type_.clone(),
                    location: expr.location.clone(),
                })
            }
            TypedExpressionKind::EnumConstruct { enum_name, variant, tag, arguments } => {
                let mut args_h = Vec::with_capacity(arguments.len());
                for arg in arguments {
                    args_h.push(self.hoist_try_expr(arg, hoisted_stmts, indent_str, return_type)?);
                }
                Ok(TypedExpression {
                    kind: TypedExpressionKind::EnumConstruct {
                        enum_name: enum_name.clone(),
                        variant: variant.clone(),
                        tag: *tag,
                        arguments: args_h,
                    },
                    type_: expr.type_.clone(),
                    location: expr.location.clone(),
                })
            }
            TypedExpressionKind::IndirectCall { callee, arguments } => {
                let callee_h = self.hoist_try_expr(callee, hoisted_stmts, indent_str, return_type)?;
                let mut args_h = Vec::with_capacity(arguments.len());
                for arg in arguments {
                    args_h.push(self.hoist_try_expr(arg, hoisted_stmts, indent_str, return_type)?);
                }
                Ok(TypedExpression {
                    kind: TypedExpressionKind::IndirectCall {
                        callee: Box::new(callee_h),
                        arguments: args_h,
                    },
                    type_: expr.type_.clone(),
                    location: expr.location.clone(),
                })
            }
            TypedExpressionKind::MethodCall { object, class_name, method, arguments } => {
                let obj_h = self.hoist_try_expr(object, hoisted_stmts, indent_str, return_type)?;
                let mut args_h = Vec::with_capacity(arguments.len());
                for arg in arguments {
                    args_h.push(self.hoist_try_expr(arg, hoisted_stmts, indent_str, return_type)?);
                }
                Ok(TypedExpression {
                    kind: TypedExpressionKind::MethodCall {
                        object: Box::new(obj_h),
                        class_name: class_name.clone(),
                        method: method.clone(),
                        arguments: args_h,
                    },
                    type_: expr.type_.clone(),
                    location: expr.location.clone(),
                })
            }
            TypedExpressionKind::TraitMethodCall { object, trait_name, method, slot, arguments } => {
                let obj_h = self.hoist_try_expr(object, hoisted_stmts, indent_str, return_type)?;
                let mut args_h = Vec::with_capacity(arguments.len());
                for arg in arguments {
                    args_h.push(self.hoist_try_expr(arg, hoisted_stmts, indent_str, return_type)?);
                }
                Ok(TypedExpression {
                    kind: TypedExpressionKind::TraitMethodCall {
                        object: Box::new(obj_h),
                        trait_name: trait_name.clone(),
                        method: method.clone(),
                        slot: *slot,
                        arguments: args_h,
                    },
                    type_: expr.type_.clone(),
                    location: expr.location.clone(),
                })
            }
            _ => Ok(expr.clone()),
        }
    }

    fn statement_always_returns(stmt: &TypedStatement) -> bool {
        match stmt {
            TypedStatement::Return { .. } => true,
            TypedStatement::Block { statements, .. } => {
                statements.iter().any(|s| Self::statement_always_returns(s))
            }
            TypedStatement::If { then_branch, else_branch: Some(else_branch), .. } => {
                Self::statement_always_returns(then_branch) && Self::statement_always_returns(else_branch)
            }
            TypedStatement::Match { arms, .. } => {
                !arms.is_empty() && arms.iter().all(|arm| Self::statement_always_returns(&arm.body))
            }
            _ => false,
        }
    }

    fn generate_match_arm_check(
        &self,
        source: &mut String,
        pattern: &TypedPattern,
        target_var: &str,
        indent: usize,
        guard: Option<&TypedExpression>,
        body: &TypedStatement,
        return_type: &Type,
        is_closure: bool,
        matched_flag: &str,
    ) -> Result<()> {
        let indent_str = "    ".repeat(indent);
        match pattern {
            TypedPattern::Wildcard { .. } => {
                source.push_str(&format!("{}if (!{}) {{\n", indent_str, matched_flag));
                if let Some(g) = guard {
                    source.push_str(&format!("{}    if (", indent_str));
                    self.generate_expression(source, g)?;
                    source.push_str(") {\n");
                    source.push_str(&format!("{}        {} = true;\n", indent_str, matched_flag));
                    self.generate_statement(source, body, indent + 2, return_type, is_closure)?;
                    source.push_str(&format!("{}    }}\n", indent_str));
                } else {
                    source.push_str(&format!("{}    {} = true;\n", indent_str, matched_flag));
                    self.generate_statement(source, body, indent + 1, return_type, is_closure)?;
                }
                source.push_str(&format!("{}}}\n", indent_str));
            }
            TypedPattern::Variable { name, type_, .. } => {
                source.push_str(&format!("{}if (!{}) {{\n", indent_str, matched_flag));
                if name != "_" {
                    source.push_str(&format!(
                        "{}    {} {} = ({}) {};\n",
                        indent_str, java_type_name(type_), escape_java_identifier(name), java_type_name(type_), target_var
                    ));
                }
                if let Some(g) = guard {
                    source.push_str(&format!("{}    if (", indent_str));
                    self.generate_expression(source, g)?;
                    source.push_str(") {\n");
                    source.push_str(&format!("{}        {} = true;\n", indent_str, matched_flag));
                    self.generate_statement(source, body, indent + 2, return_type, is_closure)?;
                    source.push_str(&format!("{}    }}\n", indent_str));
                } else {
                    source.push_str(&format!("{}    {} = true;\n", indent_str, matched_flag));
                    self.generate_statement(source, body, indent + 1, return_type, is_closure)?;
                }
                source.push_str(&format!("{}}}\n", indent_str));
            }
            TypedPattern::Literal { expr: lit, .. } => {
                source.push_str(&format!("{}if (!{} && java.util.Objects.equals({}, ", indent_str, matched_flag, target_var));
                self.generate_expression(source, lit)?;
                source.push_str(")) {\n");
                if let Some(g) = guard {
                    source.push_str(&format!("{}    if (", indent_str));
                    self.generate_expression(source, g)?;
                    source.push_str(") {\n");
                    source.push_str(&format!("{}        {} = true;\n", indent_str, matched_flag));
                    self.generate_statement(source, body, indent + 2, return_type, is_closure)?;
                    source.push_str(&format!("{}    }}\n", indent_str));
                } else {
                    source.push_str(&format!("{}    {} = true;\n", indent_str, matched_flag));
                    self.generate_statement(source, body, indent + 1, return_type, is_closure)?;
                }
                source.push_str(&format!("{}}}\n", indent_str));
            }
            TypedPattern::Range { start, end, inclusive, .. } => {
                let mut start_str = String::new();
                self.generate_expression(&mut start_str, start)?;
                let mut end_str = String::new();
                self.generate_expression(&mut end_str, end)?;

                let op = if *inclusive { "<=" } else { "<" };
                source.push_str(&format!(
                    "{}if (!{} && {} instanceof Number && ((Number){}).longValue() >= ((Number){}).longValue() && ((Number){}).longValue() {} ((Number){}).longValue()) {{\n",
                    indent_str, matched_flag, target_var, target_var, start_str, target_var, op, end_str
                ));
                if let Some(g) = guard {
                    source.push_str(&format!("{}    if (", indent_str));
                    self.generate_expression(source, g)?;
                    source.push_str(") {\n");
                    source.push_str(&format!("{}        {} = true;\n", indent_str, matched_flag));
                    self.generate_statement(source, body, indent + 2, return_type, is_closure)?;
                    source.push_str(&format!("{}    }}\n", indent_str));
                } else {
                    source.push_str(&format!("{}    {} = true;\n", indent_str, matched_flag));
                    self.generate_statement(source, body, indent + 1, return_type, is_closure)?;
                }
                source.push_str(&format!("{}}}\n", indent_str));
            }
            TypedPattern::Struct { class_name, fields, .. } => {
                let class_esc = escape_java_identifier(class_name);
                source.push_str(&format!("{}if (!{} && {} instanceof {}) {{\n", indent_str, matched_flag, target_var, class_esc));
                let inst_var = format!("__inst_{}", self.try_counter.get());
                self.try_counter.set(self.try_counter.get() + 1);
                source.push_str(&format!("{}    {} {} = ({}) {};\n", indent_str, class_esc, inst_var, class_esc, target_var));

                let mut sub_conds = Vec::new();
                let mut sub_bindings = Vec::new();
                for (f_name, f_pat) in fields {
                    let f_val = format!("{}.{}", inst_var, escape_java_identifier(f_name));
                    match f_pat {
                        TypedPattern::Variable { name, type_, .. } => {
                            if name != "_" {
                                sub_bindings.push(format!("{} {} = {};", java_type_name(type_), escape_java_identifier(name), f_val));
                            }
                        }
                        TypedPattern::Literal { expr: lit, .. } => {
                            let mut lit_str = String::new();
                            self.generate_expression(&mut lit_str, lit)?;
                            sub_conds.push(format!("java.util.Objects.equals({}, {})", f_val, lit_str));
                        }
                        _ => {}
                    }
                }

                let sub_cond_str = if sub_conds.is_empty() {
                    "true".to_string()
                } else {
                    sub_conds.join(" && ")
                };

                source.push_str(&format!("{}    if ({}) {{\n", indent_str, sub_cond_str));
                for b in sub_bindings {
                    source.push_str(&format!("{}        {}\n", indent_str, b));
                }
                if let Some(g) = guard {
                    source.push_str(&format!("{}        if (", indent_str));
                    self.generate_expression(source, g)?;
                    source.push_str(") {\n");
                    source.push_str(&format!("{}            {} = true;\n", indent_str, matched_flag));
                    self.generate_statement(source, body, indent + 3, return_type, is_closure)?;
                    source.push_str(&format!("{}        }}\n", indent_str));
                } else {
                    source.push_str(&format!("{}        {} = true;\n", indent_str, matched_flag));
                    self.generate_statement(source, body, indent + 2, return_type, is_closure)?;
                }
                source.push_str(&format!("{}    }}\n", indent_str));
                source.push_str(&format!("{}}}\n", indent_str));
            }
            TypedPattern::Slice { prefix, slice_rest, suffix, .. } => {
                let min_len = prefix.len() + suffix.len();
                let len_check = if slice_rest.is_some() {
                    format!("((java.util.List) {}).size() >= {}", target_var, min_len)
                } else {
                    format!("((java.util.List) {}).size() == {}", target_var, min_len)
                };

                source.push_str(&format!(
                    "{}if (!{} && {} instanceof java.util.List && {}) {{\n",
                    indent_str, matched_flag, target_var, len_check
                ));
                let list_var = format!("__list_{}", self.try_counter.get());
                self.try_counter.set(self.try_counter.get() + 1);
                source.push_str(&format!("{}    java.util.List {} = (java.util.List) {};\n", indent_str, list_var, target_var));

                for (i, p) in prefix.iter().enumerate() {
                    if let TypedPattern::Variable { name, type_, .. } = p {
                        if name != "_" {
                            source.push_str(&format!(
                                "{}    {} {} = ({}) {}.get({});\n",
                                indent_str, java_type_name(type_), escape_java_identifier(name), java_type_name(type_), list_var, i
                            ));
                        }
                    }
                }

                if let Some(rest_pat) = slice_rest {
                    if let TypedPattern::Variable { name, .. } = &**rest_pat {
                        if name != "_" {
                            let suff_len = suffix.len();
                            source.push_str(&format!(
                                "{}    java.util.List {} = new java.util.ArrayList({}.subList({}, {}.size() - {}));\n",
                                indent_str, escape_java_identifier(name), list_var, prefix.len(), list_var, suff_len
                            ));
                        }
                    }
                }

                if let Some(g) = guard {
                    source.push_str(&format!("{}    if (", indent_str));
                    self.generate_expression(source, g)?;
                    source.push_str(") {\n");
                    source.push_str(&format!("{}        {} = true;\n", indent_str, matched_flag));
                    self.generate_statement(source, body, indent + 2, return_type, is_closure)?;
                    source.push_str(&format!("{}    }}\n", indent_str));
                } else {
                    source.push_str(&format!("{}    {} = true;\n", indent_str, matched_flag));
                    self.generate_statement(source, body, indent + 1, return_type, is_closure)?;
                }
                source.push_str(&format!("{}}}\n", indent_str));
            }
            TypedPattern::Tuple { elements, .. } => {
                source.push_str(&format!(
                    "{}if (!{} && {} instanceof java.util.List && ((java.util.List) {}).size() == {}) {{\n",
                    indent_str, matched_flag, target_var, target_var, elements.len()
                ));
                let list_var = format!("__tuple_{}", self.try_counter.get());
                self.try_counter.set(self.try_counter.get() + 1);
                source.push_str(&format!("{}    java.util.List {} = (java.util.List) {};\n", indent_str, list_var, target_var));

                for (i, elem) in elements.iter().enumerate() {
                    if let TypedPattern::Variable { name, type_, .. } = elem {
                        if name != "_" {
                            source.push_str(&format!(
                                "{}    {} {} = ({}) {}.get({});\n",
                                indent_str, java_type_name(type_), escape_java_identifier(name), java_type_name(type_), list_var, i
                            ));
                        }
                    }
                }

                if let Some(g) = guard {
                    source.push_str(&format!("{}    if (", indent_str));
                    self.generate_expression(source, g)?;
                    source.push_str(") {\n");
                    source.push_str(&format!("{}        {} = true;\n", indent_str, matched_flag));
                    self.generate_statement(source, body, indent + 2, return_type, is_closure)?;
                    source.push_str(&format!("{}    }}\n", indent_str));
                } else {
                    source.push_str(&format!("{}    {} = true;\n", indent_str, matched_flag));
                    self.generate_statement(source, body, indent + 1, return_type, is_closure)?;
                }
                source.push_str(&format!("{}}}\n", indent_str));
            }
            TypedPattern::Variant { enum_name, variant, bindings, .. } => {
                let variant_class = format!("{}.{}", escape_java_identifier(enum_name), escape_java_identifier(variant));
                source.push_str(&format!("{}if (!{} && {} instanceof {}) {{\n", indent_str, matched_flag, target_var, variant_class));

                let arm_var = format!("__arm_val_{}", self.try_counter.get());
                self.try_counter.set(self.try_counter.get() + 1);
                source.push_str(&format!(
                    "{}    {} {} = ({}) {};\n",
                    indent_str, variant_class, arm_var, variant_class, target_var
                ));

                let mut nested_conds = Vec::new();
                let mut nested_bindings = Vec::new();

                for (i, b) in bindings.iter().enumerate() {
                    let field_access = format!("{}._{}", arm_var, i);
                    match b {
                        TypedPattern::Variable { name, type_, .. } => {
                            if name != "_" {
                                nested_bindings.push(format!("{} {} = {};", java_type_name(type_), escape_java_identifier(name), field_access));
                            }
                        }
                        TypedPattern::Literal { expr: lit, .. } => {
                            let mut lit_str = String::new();
                            self.generate_expression(&mut lit_str, lit)?;
                            nested_conds.push(format!("java.util.Objects.equals({}, {})", field_access, lit_str));
                        }
                        TypedPattern::Variant { enum_name: sub_e, variant: sub_v, bindings: sub_b, .. } => {
                            let sub_cls = format!("{}.{}", escape_java_identifier(sub_e), escape_java_identifier(sub_v));
                            nested_conds.push(format!("{} instanceof {}", field_access, sub_cls));
                            let sub_inst = format!("__sub_{}_{}", arm_var, i);
                            nested_bindings.push(format!("{} {} = ({}) {};", sub_cls, sub_inst, sub_cls, field_access));
                            for (j, sb) in sub_b.iter().enumerate() {
                                if let TypedPattern::Variable { name: sb_name, type_: sb_type, .. } = sb {
                                    if sb_name != "_" {
                                        nested_bindings.push(format!("{} {} = {}._{};", java_type_name(sb_type), escape_java_identifier(sb_name), sub_inst, j));
                                    }
                                }
                            }
                        }
                        TypedPattern::Struct { class_name: s_cls, fields: s_fields, .. } => {
                            let s_esc = escape_java_identifier(s_cls);
                            nested_conds.push(format!("{} instanceof {}", field_access, s_esc));
                            let s_inst = format!("__st_{}_{}", arm_var, i);
                            nested_bindings.push(format!("{} {} = ({}) {};", s_esc, s_inst, s_esc, field_access));
                            for (f_name, f_pat) in s_fields {
                                if let TypedPattern::Variable { name: fv_name, type_: fv_type, .. } = f_pat {
                                    if fv_name != "_" {
                                        nested_bindings.push(format!("{} {} = {}.{};", java_type_name(fv_type), escape_java_identifier(fv_name), s_inst, escape_java_identifier(f_name)));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }

                let nested_cond_str = if nested_conds.is_empty() {
                    "true".to_string()
                } else {
                    nested_conds.join(" && ")
                };

                source.push_str(&format!("{}    if ({}) {{\n", indent_str, nested_cond_str));
                for b in nested_bindings {
                    source.push_str(&format!("{}        {}\n", indent_str, b));
                }

                if let Some(g) = guard {
                    source.push_str(&format!("{}        if (", indent_str));
                    self.generate_expression(source, g)?;
                    source.push_str(") {\n");
                    source.push_str(&format!("{}            {} = true;\n", indent_str, matched_flag));
                    self.generate_statement(source, body, indent + 3, return_type, is_closure)?;
                    source.push_str(&format!("{}        }}\n", indent_str));
                } else {
                    source.push_str(&format!("{}        {} = true;\n", indent_str, matched_flag));
                    self.generate_statement(source, body, indent + 2, return_type, is_closure)?;
                }
                source.push_str(&format!("{}    }}\n", indent_str));
                source.push_str(&format!("{}}}\n", indent_str));
            }
        }
        Ok(())
    }

    fn generate_statement(&self, source: &mut String, stmt: &TypedStatement, indent: usize, return_type: &Type, is_closure: bool) -> Result<()> {
        let indent_str = "    ".repeat(indent);

        match stmt {
            TypedStatement::Block { statements, .. } => {
                for stmt in statements {
                    self.generate_statement(source, stmt, indent, return_type, is_closure)?;
                }
            }
            TypedStatement::Expression { expr, .. } => {
                let mut hoisted = String::new();
                let expr = self.hoist_try_expr(expr, &mut hoisted, &indent_str, return_type)?;
                source.push_str(&hoisted);

                if let TypedExpressionKind::Call { function, arguments } = &expr.kind {
                    if function == "println" {
                        source.push_str(&format!("{}System.out.println(", indent_str));
                        self.generate_println_args(source, arguments)?;
                        source.push_str(");\n");
                        return Ok(());
                    }
                }
                source.push_str(&indent_str);
                self.generate_expression(source, &expr)?;
                source.push_str(";\n");
            }
            TypedStatement::Let { name, type_, value, .. } => {
                let mut hoisted = String::new();
                let val = self.hoist_try_expr(value, &mut hoisted, &indent_str, return_type)?;
                source.push_str(&hoisted);

                source.push_str(&format!("{}{} {} = ", indent_str, java_type_name(type_), escape_java_identifier(name)));
                if type_ != &Type::Unknown && val.type_ == Type::Unknown {
                    let mut val_str = String::new();
                    self.generate_expression(&mut val_str, &val)?;
                    source.push_str(&java_unbox(type_, &val_str));
                } else {
                    self.generate_expression(source, &val)?;
                }
                source.push_str(";\n");
            }
            TypedStatement::Return { value, .. } => {
                if let Some(expr) = value {
                    let mut hoisted = String::new();
                    let expr = self.hoist_try_expr(expr, &mut hoisted, &indent_str, return_type)?;
                    source.push_str(&hoisted);

                    source.push_str(&format!("{}return ", indent_str));
                    if return_type != &Type::Unknown && return_type != &Type::Void && expr.type_ == Type::Unknown {
                        let mut val_str = String::new();
                        self.generate_expression(&mut val_str, &expr)?;
                        source.push_str(&java_unbox(return_type, &val_str));
                    } else {
                        self.generate_expression(source, &expr)?;
                    }
                    source.push_str(";\n");
                } else if !is_closure && return_type == &Type::Void {
                    source.push_str(&format!("{}return;\n", indent_str));
                } else {
                    source.push_str(&format!("{}return null;\n", indent_str));
                }
            }
            TypedStatement::Function { .. } => {
                // Nested function declarations aren't supported; top-level
                // functions are collected and emitted directly in `generate`.
            }
            TypedStatement::ClassDecl { .. } => {
                // Like a nested Function, a top-level-only construct;
                // top-level class declarations are collected and
                // emitted as real Java classes directly in `generate`
                // (see `generate_class`).
            }
            TypedStatement::EnumDecl { .. } => {
                // Like ClassDecl, emitted as top-level Java classes in `generate`.
            }
            TypedStatement::Match { expr, arms, .. } => {
                let mut hoisted = String::new();
                let expr = self.hoist_try_expr(expr, &mut hoisted, &indent_str, return_type)?;
                source.push_str(&hoisted);

                let id = self.try_counter.get();
                self.try_counter.set(id + 1);
                let target_var = format!("__match_val_{}", id);
                let matched_flag = format!("__matched_{}", id);

                source.push_str(&format!("{}Object {} = ", indent_str, target_var));
                self.generate_expression(source, &expr)?;
                source.push_str(";\n");
                source.push_str(&format!("{}boolean {} = false;\n", indent_str, matched_flag));

                for arm in arms {
                    self.generate_match_arm_check(
                        source,
                        &arm.pattern,
                        &target_var,
                        indent,
                        arm.guard.as_ref(),
                        &arm.body,
                        return_type,
                        is_closure,
                        &matched_flag,
                    )?;
                }

                let all_arms_return = !arms.is_empty() && arms.iter().all(|arm| Self::statement_always_returns(&arm.body));
                if all_arms_return {
                    source.push_str(&format!("{}throw new RuntimeException(\"Pattern matching failed: no arm matched \" + {});\n", indent_str, target_var));
                } else {
                    let has_unconditional_catchall = arms.iter().any(|arm| {
                        arm.guard.is_none() && matches!(arm.pattern, TypedPattern::Wildcard { .. } | TypedPattern::Variable { .. })
                    });
                    if !has_unconditional_catchall && !arms.is_empty() {
                        source.push_str(&format!("{}if (!{}) {{\n", indent_str, matched_flag));
                        source.push_str(&format!("{}    throw new RuntimeException(\"Pattern matching failed: no arm matched \" + {});\n", indent_str, target_var));
                        source.push_str(&format!("{}}}\n", indent_str));
                    }
                }
            }
            TypedStatement::FieldAssign { object, field, value, .. } => {
                let mut hoisted = String::new();
                let obj = self.hoist_try_expr(object, &mut hoisted, &indent_str, return_type)?;
                let val = self.hoist_try_expr(value, &mut hoisted, &indent_str, return_type)?;
                source.push_str(&hoisted);

                source.push_str(&indent_str);
                self.generate_expression(source, &obj)?;
                source.push_str(&format!(".{} = ", escape_java_identifier(field)));
                self.generate_expression(source, &val)?;
                source.push_str(";\n");
            }
            TypedStatement::Assign { name, target_type, value, .. } => {
                let mut hoisted = String::new();
                let val = self.hoist_try_expr(value, &mut hoisted, &indent_str, return_type)?;
                source.push_str(&hoisted);

                source.push_str(&format!("{}{} = ", indent_str, escape_java_identifier(name)));
                if target_type != &Type::Unknown && val.type_ == Type::Unknown {
                    let mut val_str = String::new();
                    self.generate_expression(&mut val_str, &val)?;
                    source.push_str(&java_unbox(target_type, &val_str));
                } else {
                    self.generate_expression(source, &val)?;
                }
                source.push_str(";\n");
            }
            TypedStatement::If { condition, then_branch, else_branch, .. } => {
                self.generate_if_chain(source, condition, then_branch, else_branch.as_deref(), indent, true, return_type, is_closure)?;
            }
            TypedStatement::While { condition, body, .. } => {
                let mut hoisted = String::new();
                let cond = self.hoist_try_expr(condition, &mut hoisted, &indent_str, return_type)?;
                source.push_str(&hoisted);

                source.push_str(&format!("{}while (", indent_str));
                self.generate_expression(source, &cond)?;
                source.push_str(") {\n");
                self.generate_statement(source, body, indent + 1, return_type, is_closure)?;
                source.push_str(&format!("{}}}\n", indent_str));
            }
            TypedStatement::For { init, condition, update, body, .. } => {
                source.push_str(&format!("{}for (", indent_str));
                self.generate_for_clause(source, init)?;
                source.push_str("; ");
                self.generate_expression(source, condition)?;
                source.push_str("; ");
                self.generate_for_clause(source, update)?;
                source.push_str(") {\n");
                self.generate_statement(source, body, indent + 1, return_type, is_closure)?;
                source.push_str(&format!("{}}}\n", indent_str));
            }
            TypedStatement::UnsafeBlock { body, .. } => {
                self.generate_statement(source, body, indent, return_type, is_closure)?;
            }
            TypedStatement::Comptime { body, .. } => {
                self.generate_statement(source, body, indent, return_type, is_closure)?;
            }
            TypedStatement::TraitDecl { .. } | TypedStatement::ImplDecl { .. } | TypedStatement::ExternFunction { .. } => {}
        }

        Ok(())
    }

    /// Renders a for-loop's init or update clause inline inside the
    /// Java `for (...)` header -- i.e. like `generate_statement`'s
    /// `Let`/`Assign` cases, but without the indent prefix or trailing
    /// `;\n` those emit as full statements. The parser guarantees a
    /// for-loop's init/update are always `Let` or `Assign`.
    fn generate_for_clause(&self, source: &mut String, stmt: &TypedStatement) -> Result<()> {
        match stmt {
            TypedStatement::Let { name, type_, value, .. } => {
                source.push_str(&format!("{} {} = ", java_type_name(type_), escape_java_identifier(name)));
                if type_ != &Type::Unknown && value.type_ == Type::Unknown {
                    let mut val_str = String::new();
                    self.generate_expression(&mut val_str, value)?;
                    source.push_str(&java_unbox(type_, &val_str));
                } else {
                    self.generate_expression(source, value)?;
                }
            }
            TypedStatement::Assign { name, target_type, value, .. } => {
                source.push_str(&format!("{} = ", escape_java_identifier(name)));
                if target_type != &Type::Unknown && value.type_ == Type::Unknown {
                    let mut val_str = String::new();
                    self.generate_expression(&mut val_str, value)?;
                    source.push_str(&java_unbox(target_type, &val_str));
                } else {
                    self.generate_expression(source, value)?;
                }
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
        condition: &TypedExpression,
        then_branch: &TypedStatement,
        else_branch: Option<&TypedStatement>,
        indent: usize,
        write_indent: bool,
        return_type: &Type,
        is_closure: bool,
    ) -> Result<()> {
        let indent_str = "    ".repeat(indent);

        let mut hoisted = String::new();
        let cond = self.hoist_try_expr(condition, &mut hoisted, &indent_str, return_type)?;
        source.push_str(&hoisted);

        if write_indent {
            source.push_str(&indent_str);
        }
        source.push_str("if (");
        self.generate_expression(source, &cond)?;
        source.push_str(") {\n");
        self.generate_statement(source, then_branch, indent + 1, return_type, is_closure)?;
        source.push_str(&format!("{}}}", indent_str));

        match else_branch {
            None => source.push('\n'),
            Some(TypedStatement::If { condition, then_branch, else_branch, .. }) => {
                source.push_str(" else ");
                self.generate_if_chain(source, condition, then_branch, else_branch.as_deref(), indent, false, return_type, is_closure)?;
            }
            Some(other) => {
                source.push_str(" else {\n");
                self.generate_statement(source, other, indent + 1, return_type, is_closure)?;
                source.push_str(&format!("{}}}\n", indent_str));
            }
        }

        Ok(())
    }

    fn generate_println_args(&self, source: &mut String, arguments: &[TypedExpression]) -> Result<()> {
        if arguments.is_empty() {
            return Ok(());
        }
        if arguments.len() == 1 {
            let arg = &arguments[0];
            match &arg.type_ {
                Type::U64 | Type::Usize => {
                    source.push_str("Long.toUnsignedString(");
                    self.generate_expression(source, arg)?;
                    source.push(')');
                    return Ok(());
                }
                Type::U32 => {
                    source.push_str("Long.toString(((long)(");
                    self.generate_expression(source, arg)?;
                    source.push_str(")) & 0xFFFFFFFFL)");
                    return Ok(());
                }
                Type::U16 => {
                    source.push_str("Integer.toString(((int)(");
                    self.generate_expression(source, arg)?;
                    source.push_str(")) & 0xFFFF)");
                    return Ok(());
                }
                Type::U8 => {
                    source.push_str("Integer.toString(((int)(");
                    self.generate_expression(source, arg)?;
                    source.push_str(")) & 0xFF)");
                    return Ok(());
                }
                _ => return self.generate_expression(source, arg),
            }
        }
        // println with several arguments concatenates them, so
        // `println(a, b)` behaves like `println(a + b)`.
        source.push('(');
        for (i, arg) in arguments.iter().enumerate() {
            if i > 0 {
                source.push_str(" + ");
            }
            self.generate_expression(source, arg)?;
        }
        source.push(')');
        Ok(())
    }

    fn generate_expression(&self, source: &mut String, expr: &TypedExpression) -> Result<()> {
        match &expr.kind {
            TypedExpressionKind::String(value) => {
                source.push_str(&format!("\"{}\"", escape_for_java_string_literal(value)));
            }
            TypedExpressionKind::Char(c) => {
                match *c {
                    '\'' => source.push_str("'\\''"),
                    '\\' => source.push_str("'\\\\'"),
                    '\n' => source.push_str("'\\n'"),
                    '\t' => source.push_str("'\\t'"),
                    '\r' => source.push_str("'\\r'"),
                    '\0' => source.push_str("'\\0'"),
                    ch if ch.is_ascii_graphic() || ch == ' ' => source.push_str(&format!("'{}'", ch)),
                    ch => source.push_str(&format!("(char) {}", ch as u32)),
                }
            }
            TypedExpressionKind::Number(value) => {
                let (raw, _) = crate::semantic::parse_numeric_literal(value);
                match &expr.type_ {
                    Type::I8 => source.push_str(&format!("((byte) {})", raw)),
                    Type::U8 => source.push_str(&format!("((int) {})", raw)),
                    Type::I16 => source.push_str(&format!("((short) {})", raw)),
                    Type::U16 => source.push_str(&format!("((int) {})", raw)),
                    Type::I32 => source.push_str(&format!("((int) {})", raw)),
                    Type::U32 => source.push_str(&format!("{}L", raw)),
                    Type::I64 | Type::U64 | Type::Isize | Type::Usize => {
                        if let Ok(v) = raw.parse::<u64>() {
                            if v > i64::MAX as u64 {
                                source.push_str(&format!("Long.parseUnsignedLong(\"{}\")", raw));
                            } else {
                                source.push_str(&format!("{}L", raw));
                            }
                        } else {
                            source.push_str(&format!("{}L", raw));
                        }
                    }
                    Type::I128 | Type::U128 => {
                        source.push_str(&format!("new java.math.BigInteger(\"{}\")", raw));
                    }
                    Type::F32 => source.push_str(&format!("{}f", raw)),
                    Type::Float | Type::F64 => {
                        if !raw.contains('.') {
                            source.push_str(&format!("{}.0", raw));
                        } else {
                            source.push_str(raw);
                        }
                    }
                    _ => {
                        if !raw.contains('.') && (raw.parse::<i64>().is_ok() && raw.parse::<i32>().is_err()) {
                            source.push_str(raw);
                            source.push('L');
                        } else {
                            source.push_str(raw);
                        }
                    }
                }
            }
            TypedExpressionKind::Identifier(name) => {
                source.push_str(&escape_java_identifier(name));
            }
            TypedExpressionKind::Boolean(value) => {
                source.push_str(&format!("{}", value));
            }
            TypedExpressionKind::Null => {
                if matches!(expr.type_, Type::RawPointer(_)) {
                    source.push_str("0L");
                } else {
                    source.push_str("null");
                }
            }
            TypedExpressionKind::Call { function, arguments } => {
                if (function == "list_get" || function == "map_get" || function == "channel_recv" || function == "recv" || function == "channel_try_recv" || function == "thread_join" || function == "fiber_join" || function == "future_await" || function == "__roze_future_await" || function == "atomic_ptr_load" || function == "atomic_load" || function == "atomic_ptr_swap" || function == "atomic_swap" || function == "subtask_get"
                    || function == "vec_deque_pop_front" || function == "vec_deque_pop_back" || function == "vec_deque_front" || function == "vec_deque_back" || function == "vec_deque_get"
                    || function == "btree_map_get" || function == "btree_map_first_key" || function == "btree_map_last_key" || function == "btree_map_remove"
                    || function == "btree_set_first" || function == "btree_set_last"
                    || function == "binary_heap_pop" || function == "binary_heap_peek"
                    || function == "index_map_get" || function == "index_map_get_index_key" || function == "index_map_get_index_value" || function == "index_map_remove"
                    || function == "ring_buffer_pop" || function == "ring_buffer_peek"
                ) && expr.type_ != Type::Unknown {
                    let mut call_buf = String::new();
                    self.generate_call(&mut call_buf, function, arguments)?;
                    source.push_str(&java_unbox(&expr.type_, &call_buf));
                } else {
                    self.generate_call(source, function, arguments)?;
                }
            }
            TypedExpressionKind::New { class_name, arguments } => {
                let class_fields: Option<Vec<Type>> = self.program.statements.iter().find_map(|s| {
                    if let TypedStatement::ClassDecl { name, fields, .. } = s {
                        if name == class_name {
                            return Some(fields.iter().map(|(_, ty)| ty.clone()).collect());
                        }
                    }
                    None
                });

                source.push_str(&format!("new {}(", escape_java_identifier(class_name)));
                for (i, arg) in arguments.iter().enumerate() {
                    if i > 0 {
                        source.push_str(", ");
                    }
                    let expected_ty = class_fields.as_ref().and_then(|f| f.get(i));
                    if let Some(exp_ty) = expected_ty {
                        if exp_ty != &Type::Unknown && arg.type_ == Type::Unknown {
                            let mut arg_str = String::new();
                            self.generate_expression(&mut arg_str, arg)?;
                            source.push_str(&java_unbox(exp_ty, &arg_str));
                            continue;
                        }
                    }
                    self.generate_expression(source, arg)?;
                }
                source.push(')');
            }
            TypedExpressionKind::FieldAccess { object, field } => {
                self.generate_expression(source, object)?;
                source.push_str(&format!(".{}", escape_java_identifier(field)));
            }
            TypedExpressionKind::Binary { left, operator, right } => {
                let is_simd = left.type_.is_simd_vector() || right.type_.is_simd_vector();
                if is_simd {
                    let vec_type = if left.type_.is_simd_vector() { &left.type_ } else { &right.type_ };
                    let (class_name, cast) = match vec_type {
                        Type::Vec4f => ("RozeVec4f", "(float)("),
                        Type::Vec8f => ("RozeVec8f", "(float)("),
                        Type::Vec4i => ("RozeVec4i", "(int)("),
                        Type::Vec16u8 => ("RozeVec16u8", "(int)("),
                        _ => unreachable!(),
                    };

                    match operator {
                        BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply | BinaryOperator::Divide => {
                            let method = match operator {
                                BinaryOperator::Add => "add",
                                BinaryOperator::Subtract => "sub",
                                BinaryOperator::Multiply => "mul",
                                BinaryOperator::Divide => "div",
                                _ => unreachable!(),
                            };
                            if !left.type_.is_simd_vector() {
                                source.push_str(&format!("{}.splat({}", class_name, cast));
                                self.generate_expression(source, left)?;
                                source.push_str(&format!(")).{}(", method));
                                self.generate_expression(source, right)?;
                                source.push(')');
                            } else if !right.type_.is_simd_vector() {
                                self.generate_expression(source, left)?;
                                source.push_str(&format!(".{}({}", method, cast));
                                self.generate_expression(source, right)?;
                                source.push_str("))");
                            } else {
                                self.generate_expression(source, left)?;
                                source.push_str(&format!(".{}(", method));
                                self.generate_expression(source, right)?;
                                source.push(')');
                            }
                        }
                        BinaryOperator::Equal => {
                            source.push_str("java.util.Objects.equals(");
                            self.generate_expression(source, left)?;
                            source.push_str(", ");
                            self.generate_expression(source, right)?;
                            source.push(')');
                        }
                        BinaryOperator::NotEqual => {
                            source.push_str("!java.util.Objects.equals(");
                            self.generate_expression(source, left)?;
                            source.push_str(", ");
                            self.generate_expression(source, right)?;
                            source.push(')');
                        }
                        _ => {}
                    }
                    return Ok(());
                }

                let is_i128 = left.type_ == Type::I128 || left.type_ == Type::U128 || right.type_ == Type::I128 || right.type_ == Type::U128;
                if is_i128 {
                    match operator {
                        BinaryOperator::Add => {
                            self.generate_expression(source, left)?;
                            source.push_str(".add(");
                            self.generate_expression(source, right)?;
                            source.push(')');
                        }
                        BinaryOperator::Subtract => {
                            self.generate_expression(source, left)?;
                            source.push_str(".subtract(");
                            self.generate_expression(source, right)?;
                            source.push(')');
                        }
                        BinaryOperator::Multiply => {
                            self.generate_expression(source, left)?;
                            source.push_str(".multiply(");
                            self.generate_expression(source, right)?;
                            source.push(')');
                        }
                        BinaryOperator::Divide => {
                            self.generate_expression(source, left)?;
                            source.push_str(".divide(");
                            self.generate_expression(source, right)?;
                            source.push(')');
                        }
                        BinaryOperator::Modulo => {
                            self.generate_expression(source, left)?;
                            source.push_str(".remainder(");
                            self.generate_expression(source, right)?;
                            source.push(')');
                        }
                        BinaryOperator::Equal => {
                            source.push_str("java.util.Objects.equals(");
                            self.generate_expression(source, left)?;
                            source.push_str(", ");
                            self.generate_expression(source, right)?;
                            source.push(')');
                        }
                        BinaryOperator::NotEqual => {
                            source.push_str("!java.util.Objects.equals(");
                            self.generate_expression(source, left)?;
                            source.push_str(", ");
                            self.generate_expression(source, right)?;
                            source.push(')');
                        }
                        BinaryOperator::LessThan => {
                            source.push('(');
                            self.generate_expression(source, left)?;
                            source.push_str(".compareTo(");
                            self.generate_expression(source, right)?;
                            source.push_str(") < 0)");
                        }
                        BinaryOperator::LessEqual => {
                            source.push('(');
                            self.generate_expression(source, left)?;
                            source.push_str(".compareTo(");
                            self.generate_expression(source, right)?;
                            source.push_str(") <= 0)");
                        }
                        BinaryOperator::GreaterThan => {
                            source.push('(');
                            self.generate_expression(source, left)?;
                            source.push_str(".compareTo(");
                            self.generate_expression(source, right)?;
                            source.push_str(") > 0)");
                        }
                        BinaryOperator::GreaterEqual => {
                            source.push('(');
                            self.generate_expression(source, left)?;
                            source.push_str(".compareTo(");
                            self.generate_expression(source, right)?;
                            source.push_str(") >= 0)");
                        }
                        BinaryOperator::And | BinaryOperator::Or => {}
                    }
                    return Ok(());
                }

                let is_u64 = left.type_ == Type::U64 || left.type_ == Type::Usize;
                if is_u64 {
                    match operator {
                        BinaryOperator::Divide => {
                            source.push_str("Long.divideUnsigned(");
                            self.generate_expression(source, left)?;
                            source.push_str(", ");
                            self.generate_expression(source, right)?;
                            source.push(')');
                            return Ok(());
                        }
                        BinaryOperator::Modulo => {
                            source.push_str("Long.remainderUnsigned(");
                            self.generate_expression(source, left)?;
                            source.push_str(", ");
                            self.generate_expression(source, right)?;
                            source.push(')');
                            return Ok(());
                        }
                        BinaryOperator::LessThan => {
                            source.push_str("(Long.compareUnsigned(");
                            self.generate_expression(source, left)?;
                            source.push_str(", ");
                            self.generate_expression(source, right)?;
                            source.push_str(") < 0)");
                            return Ok(());
                        }
                        BinaryOperator::LessEqual => {
                            source.push_str("(Long.compareUnsigned(");
                            self.generate_expression(source, left)?;
                            source.push_str(", ");
                            self.generate_expression(source, right)?;
                            source.push_str(") <= 0)");
                            return Ok(());
                        }
                        BinaryOperator::GreaterThan => {
                            source.push_str("(Long.compareUnsigned(");
                            self.generate_expression(source, left)?;
                            source.push_str(", ");
                            self.generate_expression(source, right)?;
                            source.push_str(") > 0)");
                            return Ok(());
                        }
                        BinaryOperator::GreaterEqual => {
                            source.push_str("(Long.compareUnsigned(");
                            self.generate_expression(source, left)?;
                            source.push_str(", ");
                            self.generate_expression(source, right)?;
                            source.push_str(") >= 0)");
                            return Ok(());
                        }
                        _ => {}
                    }
                }

                let is_primitive = |t: &Type| matches!(t,
                    Type::Int | Type::Bool | Type::Float | Type::Char |
                    Type::I8 | Type::U8 | Type::I16 | Type::U16 |
                    Type::I32 | Type::U32 | Type::I64 | Type::U64 |
                    Type::Isize | Type::Usize | Type::F32 | Type::F64 |
                    Type::RawPointer(_)
                );

                let use_equals_method = matches!(operator, BinaryOperator::Equal | BinaryOperator::NotEqual)
                    && (!is_primitive(&left.type_) || !is_primitive(&right.type_));

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
                    self.generate_expression(source, left)?;
                    source.push_str(", ");
                    self.generate_expression(source, right)?;
                    source.push(')');
                } else {
                    let is_string_concat = matches!(operator, BinaryOperator::Add)
                        && (left.type_ == Type::String || right.type_ == Type::String);

                    source.push('(');
                    self.generate_expression(source, left)?;
                    if is_string_concat {
                        source.push_str(" + ");
                    } else {
                        let op = match operator {
                            BinaryOperator::Add => " + ",
                            BinaryOperator::Subtract => " - ",
                            BinaryOperator::Multiply => " * ",
                            BinaryOperator::Divide => " / ",
                            BinaryOperator::Modulo => " % ",
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
                    self.generate_expression(source, right)?;
                    source.push(')');
                }
            }
            TypedExpressionKind::Unary { operator, operand } => {
                if (operand.type_ == Type::I128 || operand.type_ == Type::U128) && matches!(operator, UnaryOperator::Negate) {
                    source.push('(');
                    self.generate_expression(source, operand)?;
                    source.push_str(").negate()");
                    return Ok(());
                }
                let op = match operator {
                    UnaryOperator::Negate => "-",
                    UnaryOperator::Not => "!",
                };
                source.push_str(op);
                self.generate_expression(source, operand)?;
            }
            TypedExpressionKind::Try(inner) => {
                source.push_str("__roze_unwrap_try(");
                self.generate_expression(source, inner)?;
                source.push(')');
            }
            TypedExpressionKind::EnumConstruct { enum_name, variant, arguments, .. } => {
                source.push_str(&format!("new {}.{}(", escape_java_identifier(enum_name), escape_java_identifier(variant)));
                for (i, arg) in arguments.iter().enumerate() {
                    if i > 0 {
                        source.push_str(", ");
                    }
                    self.generate_expression(source, arg)?;
                }
                source.push(')');
            }
            TypedExpressionKind::FunctionRef(name) => {
                let func_info = self.program.statements.iter().find_map(|s| {
                    if let TypedStatement::Function { name: fn_name, params, return_type, .. } = s {
                        if fn_name == name { return Some((params.clone(), return_type.clone())); }
                    }
                    None
                });
                if let Some((params, ret_ty)) = func_info {
                    if ret_ty == Type::Void {
                        source.push_str("((RozeFn) (__args) -> { ");
                        source.push_str(&escape_java_identifier(name));
                        source.push('(');
                        for (i, p) in params.iter().enumerate() {
                            if i > 0 { source.push_str(", "); }
                            source.push_str(&java_unbox(&p.type_, &format!("__args[{}]", i)));
                        }
                        source.push_str("); return null; })");
                    } else {
                        source.push_str("((RozeFn) (__args) -> ");
                        source.push_str(&escape_java_identifier(name));
                        source.push('(');
                        for (i, p) in params.iter().enumerate() {
                            if i > 0 { source.push_str(", "); }
                            source.push_str(&java_unbox(&p.type_, &format!("__args[{}]", i)));
                        }
                        source.push_str("))");
                    }
                } else {
                    source.push_str("((RozeFn) (__args) -> ");
                    source.push_str(&escape_java_identifier(name));
                    source.push_str("())");
                }
            }
            TypedExpressionKind::Closure { params, return_type, captures: _, body, .. } => {
                source.push_str("((RozeFn) (__args) -> {\n");
                for (i, (p_name, p_ty)) in params.iter().enumerate() {
                    source.push_str(&format!(
                        "    {} {} = {};\n",
                        java_type_name(p_ty),
                        escape_java_identifier(p_name),
                        java_unbox(p_ty, &format!("__args[{}]", i))
                    ));
                }
                self.generate_statement(source, body, 1, return_type, true)?;
                if !Self::statement_always_returns(body) {
                    source.push_str("    return null;\n");
                }
                source.push_str("})");
            }
            TypedExpressionKind::IndirectCall { callee, arguments } => {
                let mut call_str = String::new();
                call_str.push_str("((RozeFn) (");
                self.generate_expression(&mut call_str, callee)?;
                call_str.push_str(")).apply(");
                for (i, arg) in arguments.iter().enumerate() {
                    if i > 0 { call_str.push_str(", "); }
                    self.generate_expression(&mut call_str, arg)?;
                }
                call_str.push(')');
                if expr.type_ == Type::Void {
                    source.push_str(&call_str);
                } else {
                    source.push_str(&java_unbox(&expr.type_, &call_str));
                }
            }
            TypedExpressionKind::MethodCall { object, method, arguments, .. } |
            TypedExpressionKind::TraitMethodCall { object, method, arguments, .. } => {
                source.push('(');
                self.generate_expression(source, object)?;
                source.push(')');
                source.push_str(&format!(".{}(", escape_java_identifier(method)));
                for (i, arg) in arguments.iter().enumerate() {
                    if i > 0 {
                        source.push_str(", ");
                    }
                    self.generate_expression(source, arg)?;
                }
                source.push(')');
            }
            TypedExpressionKind::Await(inner) => {
                let mut await_buf = String::new();
                await_buf.push_str("__roze_future_await(");
                self.generate_expression(&mut await_buf, inner)?;
                await_buf.push(')');
                if expr.type_ != Type::Unknown && expr.type_ != Type::Void {
                    source.push_str(&java_unbox(&expr.type_, &await_buf));
                } else {
                    source.push_str(&await_buf);
                }
            }
        }

        Ok(())
    }

    /// Generates a call, rewriting Core (string/math) intrinsics to real
    /// JVM standard-library calls, since Roze has no method-call syntax of
    /// its own yet to express `s.length()` directly.
    fn generate_call(&self, source: &mut String, name: &str, arguments: &[TypedExpression]) -> Result<()> {
        match name {
            "string_length" if arguments.len() == 1 => {
                source.push_str("((String)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")).length()");
            }
            "string_to_upper" if arguments.len() == 1 => {
                source.push_str("((String)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")).toUpperCase()");
            }
            "string_to_lower" if arguments.len() == 1 => {
                source.push_str("((String)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")).toLowerCase()");
            }
            "string_concat" if arguments.len() == 2 => {
                source.push('(');
                self.generate_expression(source, &arguments[0])?;
                source.push_str(" + ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "substring" | "string_substring" if arguments.len() == 3 => {
                source.push_str("((String)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")).substring(");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push(')');
            }
            "string_contains" if arguments.len() == 2 => {
                source.push_str("((String)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")).contains((CharSequence)(");
                self.generate_expression(source, &arguments[1])?;
                source.push_str("))");
            }
            "string_starts_with" if arguments.len() == 2 => {
                source.push_str("((String)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")).startsWith((String)(");
                self.generate_expression(source, &arguments[1])?;
                source.push_str("))");
            }
            "string_ends_with" if arguments.len() == 2 => {
                source.push_str("((String)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")).endsWith((String)(");
                self.generate_expression(source, &arguments[1])?;
                source.push_str("))");
            }
            "abs" if arguments.len() == 1 => {
                source.push_str("Math.abs(");
                self.generate_numeric_arg(source, &arguments[0])?;
                source.push(')');
            }
            "max" if arguments.len() == 2 => {
                source.push_str("Math.max(");
                self.generate_numeric_arg(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_numeric_arg(source, &arguments[1])?;
                source.push(')');
            }
            "min" if arguments.len() == 2 => {
                source.push_str("Math.min(");
                self.generate_numeric_arg(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_numeric_arg(source, &arguments[1])?;
                source.push(')');
            }
            "to_string" if arguments.len() == 1 => {
                source.push_str("String.valueOf(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "to_int" if arguments.len() == 1 => {
                source.push_str("__roze_to_int(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "is_number" if arguments.len() == 1 => {
                // Integer and Double both extend java.lang.Number, and
                // those are the only two boxed numeric types Roze's own
                // runtime ever produces -- checking the common
                // superclass once avoids evaluating the argument twice
                // (which duplicating the instanceof check per-type
                // would do, wrongly re-running any side effects).
                source.push_str("((Object)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(") instanceof Number)");
            }
            "is_string" if arguments.len() == 1 => {
                source.push_str("((Object)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(") instanceof String)");
            }

            // ---- Fixed-Width Conversions ----
            "i8" if arguments.len() == 1 => {
                source.push_str("((byte) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str("))");
            }
            "u8" if arguments.len() == 1 => {
                source.push_str("(((int) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")) & 0xFF)");
            }
            "i16" if arguments.len() == 1 => {
                source.push_str("((short) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str("))");
            }
            "u16" if arguments.len() == 1 => {
                source.push_str("(((int) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")) & 0xFFFF)");
            }
            "i32" if arguments.len() == 1 => {
                source.push_str("((int) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str("))");
            }
            "u32" if arguments.len() == 1 => {
                source.push_str("(((long) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")) & 0xFFFFFFFFL)");
            }
            "i64" | "isize" if arguments.len() == 1 => {
                source.push_str("((long) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str("))");
            }
            "u64" | "usize" if arguments.len() == 1 => {
                source.push_str("((long) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str("))");
            }
            "i128" if arguments.len() == 1 => {
                source.push_str("__roze_to_i128(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "u128" if arguments.len() == 1 => {
                source.push_str("__roze_to_u128(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "f32" if arguments.len() == 1 => {
                source.push_str("((float) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str("))");
            }
            "f64" | "float" if arguments.len() == 1 => {
                source.push_str("((double) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str("))");
            }
            "char" if arguments.len() == 1 => {
                source.push_str("((char) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str("))");
            }
            "int" if arguments.len() == 1 => {
                source.push_str("((int) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str("))");
            }

            // ---- Fixed-Width Bounds ----
            "i8_min" if arguments.is_empty() => source.push_str("((byte) -128)"),
            "i8_max" if arguments.is_empty() => source.push_str("((byte) 127)"),
            "u8_min" if arguments.is_empty() => source.push_str("0"),
            "u8_max" if arguments.is_empty() => source.push_str("255"),
            "i16_min" if arguments.is_empty() => source.push_str("((short) -32768)"),
            "i16_max" if arguments.is_empty() => source.push_str("((short) 32767)"),
            "u16_min" if arguments.is_empty() => source.push_str("0"),
            "u16_max" if arguments.is_empty() => source.push_str("65535"),
            "i32_min" if arguments.is_empty() => source.push_str("Integer.MIN_VALUE"),
            "i32_max" if arguments.is_empty() => source.push_str("Integer.MAX_VALUE"),
            "u32_min" if arguments.is_empty() => source.push_str("0L"),
            "u32_max" if arguments.is_empty() => source.push_str("4294967295L"),
            "i64_min" | "isize_min" if arguments.is_empty() => source.push_str("Long.MIN_VALUE"),
            "i64_max" | "isize_max" if arguments.is_empty() => source.push_str("Long.MAX_VALUE"),
            "u64_min" | "usize_min" if arguments.is_empty() => source.push_str("0L"),
            "u64_max" | "usize_max" if arguments.is_empty() => source.push_str("-1L"),
            "i128_min" if arguments.is_empty() => source.push_str("__roze_i128_min()"),
            "i128_max" if arguments.is_empty() => source.push_str("__roze_i128_max()"),
            "u128_min" if arguments.is_empty() => source.push_str("__roze_u128_min()"),
            "u128_max" if arguments.is_empty() => source.push_str("__roze_u128_max()"),
            "f32_min" if arguments.is_empty() => source.push_str("(-Float.MAX_VALUE)"),
            "f32_max" if arguments.is_empty() => source.push_str("Float.MAX_VALUE"),
            "f64_min" if arguments.is_empty() => source.push_str("(-Double.MAX_VALUE)"),
            "f64_max" if arguments.is_empty() => source.push_str("Double.MAX_VALUE"),

            // ---- Wrapping & Saturating Arithmetic ----
            "wrapping_add" | "wrapping_sub" | "wrapping_mul"
            | "saturating_add" | "saturating_sub" | "saturating_mul" if arguments.len() == 2 => {
                let suffix = match &arguments[0].type_ {
                    Type::I8 => "i8",
                    Type::U8 => "u8",
                    Type::I16 => "i16",
                    Type::U16 => "u16",
                    Type::I32 => "i32",
                    Type::U32 => "u32",
                    Type::I64 | Type::Int | Type::Isize => "i64",
                    Type::U64 | Type::Usize => "u64",
                    Type::I128 => "i128",
                    Type::U128 => "u128",
                    _ => "i64",
                };
                source.push_str(&format!("__roze_{}_{}(", name, suffix));
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }

            // ---- Hardware SIMD Vector Intrinsics (M2-T02) ----
            // Vec4f
            "Vec4f" | "vec4f" => {
                if arguments.is_empty() {
                    source.push_str("RozeVec4f.zero()");
                } else if arguments.len() == 1 {
                    source.push_str("RozeVec4f.splat((float)(");
                    self.generate_expression(source, &arguments[0])?;
                    source.push_str("))");
                } else if arguments.len() == 4 {
                    source.push_str("new RozeVec4f(");
                    for (i, arg) in arguments.iter().enumerate() {
                        if i > 0 { source.push_str(", "); }
                        source.push_str("(float)(");
                        self.generate_expression(source, arg)?;
                        source.push(')');
                    }
                    source.push(')');
                }
            }
            "Vec4f_splat" | "vec4f_splat" if arguments.len() == 1 => {
                source.push_str("RozeVec4f.splat((float)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str("))");
            }
            "Vec4f_zero" | "vec4f_zero" if arguments.is_empty() => {
                source.push_str("RozeVec4f.zero()");
            }
            "vec4f_add" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".add(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec4f_sub" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".sub(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec4f_mul" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".mul(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec4f_div" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".div(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec4f_dot" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".dot(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec4f_sum" if arguments.len() == 1 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".sum()");
            }
            "vec4f_min" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".min(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec4f_max" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".max(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec4f_sqrt" if arguments.len() == 1 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".sqrt()");
            }
            "vec4f_abs" if arguments.len() == 1 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".abs()");
            }
            "vec4f_fma" if arguments.len() == 3 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".fma(");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push(')');
            }
            "vec4f_get" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".get(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec4f_set" if arguments.len() == 3 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".set(");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", (float)(");
                self.generate_expression(source, &arguments[2])?;
                source.push_str("))");
            }
            "vec4f_load" if arguments.len() == 1 => {
                source.push_str("__roze_vec4f_load(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "vec4f_store" if arguments.len() == 2 => {
                source.push_str("__roze_vec4f_store(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }

            // Vec8f
            "Vec8f" | "vec8f" => {
                if arguments.is_empty() {
                    source.push_str("RozeVec8f.zero()");
                } else if arguments.len() == 1 {
                    source.push_str("RozeVec8f.splat((float)(");
                    self.generate_expression(source, &arguments[0])?;
                    source.push_str("))");
                } else if arguments.len() == 8 {
                    source.push_str("new RozeVec8f(");
                    for (i, arg) in arguments.iter().enumerate() {
                        if i > 0 { source.push_str(", "); }
                        source.push_str("(float)(");
                        self.generate_expression(source, arg)?;
                        source.push(')');
                    }
                    source.push(')');
                }
            }
            "Vec8f_splat" | "vec8f_splat" if arguments.len() == 1 => {
                source.push_str("RozeVec8f.splat((float)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str("))");
            }
            "Vec8f_zero" | "vec8f_zero" if arguments.is_empty() => {
                source.push_str("RozeVec8f.zero()");
            }
            "vec8f_add" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".add(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec8f_sub" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".sub(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec8f_mul" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".mul(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec8f_div" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".div(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec8f_dot" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".dot(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec8f_sum" if arguments.len() == 1 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".sum()");
            }
            "vec8f_min" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".min(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec8f_max" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".max(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec8f_sqrt" if arguments.len() == 1 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".sqrt()");
            }
            "vec8f_abs" if arguments.len() == 1 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".abs()");
            }
            "vec8f_fma" if arguments.len() == 3 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".fma(");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push(')');
            }
            "vec8f_get" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".get(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec8f_set" if arguments.len() == 3 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".set(");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", (float)(");
                self.generate_expression(source, &arguments[2])?;
                source.push_str("))");
            }
            "vec8f_load" if arguments.len() == 1 => {
                source.push_str("__roze_vec8f_load(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "vec8f_store" if arguments.len() == 2 => {
                source.push_str("__roze_vec8f_store(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }

            // Vec4i
            "Vec4i" | "vec4i" => {
                if arguments.is_empty() {
                    source.push_str("RozeVec4i.zero()");
                } else if arguments.len() == 1 {
                    source.push_str("RozeVec4i.splat((int)(");
                    self.generate_expression(source, &arguments[0])?;
                    source.push_str("))");
                } else if arguments.len() == 4 {
                    source.push_str("new RozeVec4i(");
                    for (i, arg) in arguments.iter().enumerate() {
                        if i > 0 { source.push_str(", "); }
                        source.push_str("(int)(");
                        self.generate_expression(source, arg)?;
                        source.push(')');
                    }
                    source.push(')');
                }
            }
            "Vec4i_splat" | "vec4i_splat" if arguments.len() == 1 => {
                source.push_str("RozeVec4i.splat((int)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str("))");
            }
            "Vec4i_zero" | "vec4i_zero" if arguments.is_empty() => {
                source.push_str("RozeVec4i.zero()");
            }
            "vec4i_add" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".add(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec4i_sub" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".sub(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec4i_mul" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".mul(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec4i_div" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".div(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec4i_sum" if arguments.len() == 1 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".sum()");
            }
            "vec4i_min" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".min(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec4i_max" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".max(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec4i_abs" if arguments.len() == 1 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".abs()");
            }
            "vec4i_get" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".get(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec4i_set" if arguments.len() == 3 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".set(");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", (int)(");
                self.generate_expression(source, &arguments[2])?;
                source.push_str("))");
            }
            "vec4i_load" if arguments.len() == 1 => {
                source.push_str("__roze_vec4i_load(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "vec4i_store" if arguments.len() == 2 => {
                source.push_str("__roze_vec4i_store(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }

            // Vec16u8
            "Vec16u8" | "vec16u8" => {
                if arguments.is_empty() {
                    source.push_str("RozeVec16u8.zero()");
                } else if arguments.len() == 1 {
                    source.push_str("RozeVec16u8.splat((int)(");
                    self.generate_expression(source, &arguments[0])?;
                    source.push_str("))");
                } else if arguments.len() == 16 {
                    source.push_str("new RozeVec16u8(");
                    for (i, arg) in arguments.iter().enumerate() {
                        if i > 0 { source.push_str(", "); }
                        source.push_str("(int)(");
                        self.generate_expression(source, arg)?;
                        source.push(')');
                    }
                    source.push(')');
                }
            }
            "Vec16u8_splat" | "vec16u8_splat" if arguments.len() == 1 => {
                source.push_str("RozeVec16u8.splat((int)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str("))");
            }
            "Vec16u8_zero" | "vec16u8_zero" if arguments.is_empty() => {
                source.push_str("RozeVec16u8.zero()");
            }
            "vec16u8_add" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".add(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec16u8_sub" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".sub(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec16u8_mul" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".mul(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec16u8_saturating_add" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".saturating_add(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec16u8_saturating_sub" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".saturating_sub(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec16u8_sum" if arguments.len() == 1 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".sum()");
            }
            "vec16u8_min" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".min(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec16u8_max" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".max(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec16u8_get" if arguments.len() == 2 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".get(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vec16u8_set" if arguments.len() == 3 => {
                self.generate_expression(source, &arguments[0])?;
                source.push_str(".set(");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", (int)(");
                self.generate_expression(source, &arguments[2])?;
                source.push_str("))");
            }
            "vec16u8_load" if arguments.len() == 1 => {
                source.push_str("__roze_vec16u8_load(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "vec16u8_store" if arguments.len() == 2 => {
                source.push_str("__roze_vec16u8_store(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }

            // ---- Collections: List ----
            "list_new" if arguments.is_empty() => {
                source.push_str("new java.util.ArrayList()");
            }
            "list_push" if arguments.len() == 2 => {
                self.generate_list_receiver(source, &arguments[0])?;
                source.push_str(".add(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "list_get" if arguments.len() == 2 => {
                self.generate_list_receiver(source, &arguments[0])?;
                source.push_str(".get(");
                self.generate_index_arg(source, &arguments[1])?;
                source.push(')');
            }
            "list_set" if arguments.len() == 3 => {
                self.generate_list_receiver(source, &arguments[0])?;
                source.push_str(".set(");
                self.generate_index_arg(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push(')');
            }
            "list_remove" if arguments.len() == 2 => {
                // Must be a genuine primitive int (see generate_index_arg):
                // List.remove has both remove(int) and remove(Object)
                // overloads, and a boxed Integer argument binds to
                // remove(Object) -- remove-by-equality, not by index.
                self.generate_list_receiver(source, &arguments[0])?;
                source.push_str(".remove(");
                self.generate_index_arg(source, &arguments[1])?;
                source.push(')');
            }
            "list_length" if arguments.len() == 1 => {
                self.generate_list_receiver(source, &arguments[0])?;
                source.push_str(".size()");
            }
            "list_is_empty" if arguments.len() == 1 => {
                self.generate_list_receiver(source, &arguments[0])?;
                source.push_str(".isEmpty()");
            }

            // ---- Collections: Map ----
            "map_new" if arguments.is_empty() => {
                source.push_str("new java.util.HashMap()");
            }
            "map_put" | "map_set" if arguments.len() == 3 => {
                self.generate_map_receiver(source, &arguments[0])?;
                source.push_str(".put(");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push(')');
            }
            "map_get" if arguments.len() == 2 => {
                self.generate_map_receiver(source, &arguments[0])?;
                source.push_str(".get(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "map_has" if arguments.len() == 2 => {
                self.generate_map_receiver(source, &arguments[0])?;
                source.push_str(".containsKey(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "map_remove" if arguments.len() == 2 => {
                self.generate_map_receiver(source, &arguments[0])?;
                source.push_str(".remove(");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "map_size" if arguments.len() == 1 => {
                self.generate_map_receiver(source, &arguments[0])?;
                source.push_str(".size()");
            }
            "map_is_empty" if arguments.len() == 1 => {
                self.generate_map_receiver(source, &arguments[0])?;
                source.push_str(".isEmpty()");
            }
            "map_keys" if arguments.len() == 1 => {
                source.push_str("new java.util.ArrayList(");
                self.generate_map_receiver(source, &arguments[0])?;
                source.push_str(".keySet())");
            }

            // ---- IO: file ----
            // Each delegates to a fixed helper method (see
            // IO_HELPER_METHODS) that wraps the underlying checked
            // exception and rethrows unchecked.
            "read_file" if arguments.len() == 1 => {
                source.push_str("__roze_read_file(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "write_file" if arguments.len() == 2 => {
                source.push_str("__roze_write_file(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "append_file" if arguments.len() == 2 => {
                source.push_str("__roze_append_file(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "file_exists" if arguments.len() == 1 => {
                // Files.exists doesn't throw, so no helper needed.
                source.push_str("java.nio.file.Files.exists(java.nio.file.Paths.get(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str("))");
            }
            "delete_file" if arguments.len() == 1 => {
                source.push_str("__roze_delete_file(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "read_lines" if arguments.len() == 1 => {
                source.push_str("__roze_read_lines(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }

            // ---- IO: network ----
            "http_get" if arguments.len() == 1 => {
                source.push_str("__roze_http_get(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "http_post" if arguments.len() == 2 => {
                source.push_str("__roze_http_post(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "http_request" if arguments.len() == 4 => {
                source.push_str("__roze_http_request(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[3])?;
                source.push(')');
            }

            // ---- Web: JSON ----
            "json_encode" if arguments.len() == 1 => {
                source.push_str("__roze_json_encode(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "json_decode" if arguments.len() == 1 => {
                source.push_str("__roze_json_decode(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }

            // ---- Web: HTTP server ----
            "http_server_start" if arguments.len() == 1 => {
                source.push_str("__roze_http_server_start(");
                self.generate_int_arg(source, &arguments[0])?;
                source.push(')');
            }
            "http_server_listen" if arguments.len() == 2 => {
                source.push_str("__roze_http_server_listen(");
                self.generate_int_arg(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "http_server_accept" if arguments.len() == 1 => {
                source.push_str("__roze_http_server_accept((java.net.ServerSocket)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str("))");
            }
            "http_server_respond" if arguments.len() == 3 => {
                source.push_str("__roze_http_server_respond(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_int_arg(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push(')');
            }
            "http_server_respond_json" if arguments.len() == 3 => {
                source.push_str("__roze_http_server_respond_json(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_int_arg(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push(')');
            }
            "http_server_respond_headers" if arguments.len() == 4 => {
                source.push_str("__roze_http_server_respond_headers(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_int_arg(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[3])?;
                source.push(')');
            }
            "url_route_match" if arguments.len() == 2 => {
                source.push_str("__roze_url_route_match(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "http_server_stop" if arguments.len() == 1 => {
                source.push_str("__roze_http_server_stop(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }

            // ---- Database (SQL) ----
            "sql_connect" if arguments.len() == 1 => {
                source.push_str("__roze_sql_connect(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "sql_query" if arguments.len() == 2 => {
                source.push_str("__roze_sql_query(((java.sql.Connection)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")), ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "sql_execute" if arguments.len() == 2 => {
                source.push_str("__roze_sql_execute(((java.sql.Connection)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")), ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "sql_close" if arguments.len() == 1 => {
                source.push_str("__roze_sql_close((java.sql.Connection)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str("))");
            }

            // ---- Database Connection Pooling (ADV-03) ----
            "db_pool_create" if arguments.len() == 2 => {
                source.push_str("__roze_db_pool_create(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "db_pool_get" if arguments.len() == 1 => {
                source.push_str("__roze_db_pool_get(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "db_pool_release" if arguments.len() == 2 => {
                source.push_str("__roze_db_pool_release(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "db_pool_close" if arguments.len() == 1 => {
                source.push_str("__roze_db_pool_close(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "db_pool_stats" if arguments.len() == 1 => {
                source.push_str("__roze_db_pool_stats(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "db_pool_query" if arguments.len() == 2 => {
                source.push_str("__roze_db_pool_query(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "db_pool_execute" if arguments.len() == 2 => {
                source.push_str("__roze_db_pool_execute(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }

            // ---- JWT Security (ADV-03) ----
            "jwt_sign" if arguments.len() == 2 => {
                source.push_str("__roze_jwt_sign(((java.util.Map)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")), ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "jwt_verify" if arguments.len() == 2 => {
                source.push_str("__roze_jwt_verify(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "jwt_decode" if arguments.len() == 1 => {
                source.push_str("__roze_jwt_decode(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "jwt_decode_header" if arguments.len() == 1 => {
                source.push_str("__roze_jwt_decode_header(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }

            // ---- Time (ADV-03) ----
            "time_now" if arguments.is_empty() => {
                source.push_str("__roze_time_now()");
            }
            "time_now_millis" if arguments.is_empty() => {
                source.push_str("__roze_time_now_millis()");
            }

            // ---- Option ----
            "Some" | "option_some" if arguments.len() == 1 => {
                source.push_str("RozeOption.some(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "None" | "option_none" if arguments.is_empty() => {
                source.push_str("RozeOption.none()");
            }
            "is_some" | "option_is_some" if arguments.len() == 1 => {
                source.push_str("(((RozeOption) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")).isSome())");
            }
            "is_none" | "option_is_none" if arguments.len() == 1 => {
                source.push_str("(((RozeOption) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")).isNone())");
            }
            "option_unwrap" if arguments.len() == 1 => {
                source.push_str("(((RozeOption) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")).unwrap())");
            }
            "option_unwrap_or" if arguments.len() == 2 => {
                source.push_str("(((RozeOption) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")).unwrapOr(");
                self.generate_expression(source, &arguments[1])?;
                source.push_str("))");
            }

            // ---- Result ----
            "Ok" | "result_ok" if arguments.len() == 1 => {
                source.push_str("RozeResult.ok(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "Err" | "result_err" if arguments.len() == 1 => {
                source.push_str("RozeResult.err(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "is_ok" | "result_is_ok" if arguments.len() == 1 => {
                source.push_str("(((RozeResult) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")).isOk())");
            }
            "is_err" | "result_is_err" if arguments.len() == 1 => {
                source.push_str("(((RozeResult) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")).isErr())");
            }
            "unwrap" | "result_unwrap" if arguments.len() == 1 => {
                source.push_str("__roze_unwrap(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "unwrap_err" | "result_unwrap_err" if arguments.len() == 1 => {
                source.push_str("(((RozeResult) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(")).unwrapErr())");
            }
            "unwrap_or" | "result_unwrap_or" if arguments.len() == 2 => {
                source.push_str("__roze_unwrap_or(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }

            // ---- Safe IO ----
            "read_file_result" if arguments.len() == 1 => {
                source.push_str("__roze_read_file_result(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "write_file_result" if arguments.len() == 2 => {
                source.push_str("__roze_write_file_result(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "append_file_result" if arguments.len() == 2 => {
                source.push_str("__roze_append_file_result(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "delete_file_result" if arguments.len() == 1 => {
                source.push_str("__roze_delete_file_result(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "read_lines_result" if arguments.len() == 1 => {
                source.push_str("__roze_read_lines_result(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "http_get_result" if arguments.len() == 1 => {
                source.push_str("__roze_http_get_result(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "http_post_result" if arguments.len() == 2 => {
                source.push_str("__roze_http_post_result(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "to_int_result" if arguments.len() == 1 => {
                source.push_str("__roze_to_int_result(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }

            // ---- Safe Collections ----
            "list_get_option" if arguments.len() == 2 => {
                source.push_str("__roze_list_get_option(");
                self.generate_list_receiver(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_int_arg(source, &arguments[1])?;
                source.push(')');
            }
            "map_get_option" if arguments.len() == 2 => {
                source.push_str("__roze_map_get_option(");
                self.generate_map_receiver(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }

            // ---- Concurrency: Threads & Channels (MED-03) ----
            "thread_spawn" if arguments.len() == 1 => {
                source.push_str("__roze_thread_spawn(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "thread_join" if arguments.len() == 1 => {
                source.push_str("__roze_thread_join(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "thread_sleep" if arguments.len() == 1 => {
                source.push_str("__roze_thread_sleep(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "thread_current_id" if arguments.is_empty() => {
                source.push_str("__roze_thread_current_id()");
            }

            // ---- Modern Concurrency: M:N Green Threads / Virtual Threads (M1-T04) ----
            "fiber_spawn" if arguments.len() == 1 => {
                source.push_str("__roze_fiber_spawn(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "fiber_join" if arguments.len() == 1 => {
                source.push_str("__roze_fiber_join(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "fiber_yield" if arguments.is_empty() => {
                source.push_str("__roze_fiber_yield()");
            }
            "fiber_sleep" if arguments.len() == 1 => {
                source.push_str("__roze_fiber_sleep(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "fiber_id" if arguments.is_empty() => {
                source.push_str("__roze_fiber_id()");
            }
            "fiber_count" if arguments.is_empty() => {
                source.push_str("__roze_fiber_count()");
            }
            "fiber_stats" if arguments.is_empty() => {
                source.push_str("__roze_fiber_stats()");
            }
            "future_spawn" | "__roze_async_spawn" if arguments.len() == 1 => {
                source.push_str("__roze_async_spawn(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "future_await" | "__roze_future_await" if arguments.len() == 1 => {
                source.push_str("__roze_future_await(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "future_poll" | "__roze_future_poll" if arguments.len() == 1 => {
                source.push_str("__roze_future_poll(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "future_ready" | "__roze_future_ready" if arguments.len() == 1 => {
                source.push_str("__roze_future_ready(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "async_sleep" | "__roze_async_sleep" if arguments.len() == 1 => {
                source.push_str("__roze_async_sleep((int)(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str("))");
            }
            "channel_new" => {
                source.push_str("__roze_channel_new(");
                if arguments.len() == 1 {
                    self.generate_expression(source, &arguments[0])?;
                } else {
                    source.push_str("64");
                }
                source.push(')');
            }
            "channel_unbounded" if arguments.is_empty() => {
                source.push_str("__roze_channel_unbounded()");
            }
            "channel_send" | "send" if arguments.len() == 2 => {
                source.push_str("__roze_channel_send(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "channel_try_send" if arguments.len() == 2 => {
                source.push_str("__roze_channel_try_send(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "channel_recv" | "recv" if arguments.len() == 1 => {
                source.push_str("__roze_channel_recv(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "channel_try_recv" if arguments.len() == 1 => {
                source.push_str("__roze_channel_try_recv(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "channel_close" if arguments.len() == 1 => {
                source.push_str("__roze_channel_close(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "channel_is_closed" if arguments.len() == 1 => {
                source.push_str("__roze_channel_is_closed(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "channel_len" if arguments.len() == 1 => {
                source.push_str("__roze_channel_len(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "channel_capacity" if arguments.len() == 1 => {
                source.push_str("__roze_channel_capacity(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }

            // Mutex & RwLock (M1-T06)
            "mutex_new" if arguments.is_empty() => {
                source.push_str("__roze_mutex_new()");
            }
            "mutex_lock" if arguments.len() == 1 => {
                source.push_str("__roze_mutex_lock(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "mutex_unlock" if arguments.len() == 1 => {
                source.push_str("__roze_mutex_unlock(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "mutex_try_lock" if arguments.len() == 1 => {
                source.push_str("__roze_mutex_try_lock(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }

            "rwlock_new" if arguments.is_empty() => {
                source.push_str("__roze_rwlock_new()");
            }
            "rwlock_read_lock" if arguments.len() == 1 => {
                source.push_str("__roze_rwlock_read_lock(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "rwlock_read_unlock" if arguments.len() == 1 => {
                source.push_str("__roze_rwlock_read_unlock(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "rwlock_write_lock" if arguments.len() == 1 => {
                source.push_str("__roze_rwlock_write_lock(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "rwlock_write_unlock" if arguments.len() == 1 => {
                source.push_str("__roze_rwlock_write_unlock(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "rwlock_try_read_lock" if arguments.len() == 1 => {
                source.push_str("__roze_rwlock_try_read_lock(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "rwlock_try_write_lock" if arguments.len() == 1 => {
                source.push_str("__roze_rwlock_try_write_lock(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }

            // Atomics (M1-T06)
            "atomic_bool_new" if arguments.len() == 1 => {
                source.push_str("__roze_atomic_bool_new(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "atomic_bool_load" if arguments.len() == 1 => {
                source.push_str("__roze_atomic_bool_load(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "atomic_bool_store" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_bool_store(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "atomic_bool_swap" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_bool_swap(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "atomic_bool_cas" if arguments.len() == 3 => {
                source.push_str("__roze_atomic_bool_cas(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push(')');
            }

            "atomic_i64_new" if arguments.len() == 1 => {
                source.push_str("__roze_atomic_i64_new(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "atomic_i64_load" if arguments.len() == 1 => {
                source.push_str("__roze_atomic_i64_load(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "atomic_i64_store" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_i64_store(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "atomic_i64_swap" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_i64_swap(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "atomic_i64_cas" if arguments.len() == 3 => {
                source.push_str("__roze_atomic_i64_cas(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push(')');
            }
            "atomic_i64_add" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_i64_add(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "atomic_i64_sub" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_i64_sub(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "atomic_i64_and" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_i64_and(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "atomic_i64_or" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_i64_or(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "atomic_i64_xor" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_i64_xor(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }

            "atomic_usize_new" if arguments.len() == 1 => {
                source.push_str("__roze_atomic_usize_new(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "atomic_usize_load" if arguments.len() == 1 => {
                source.push_str("__roze_atomic_usize_load(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "atomic_usize_store" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_usize_store(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "atomic_usize_swap" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_usize_swap(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "atomic_usize_cas" if arguments.len() == 3 => {
                source.push_str("__roze_atomic_usize_cas(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push(')');
            }
            "atomic_usize_add" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_usize_add(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "atomic_usize_sub" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_usize_sub(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }

            "atomic_ptr_new" if arguments.len() == 1 => {
                source.push_str("__roze_atomic_ptr_new(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "atomic_ptr_load" if arguments.len() == 1 => {
                source.push_str("__roze_atomic_ptr_load(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "atomic_ptr_store" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_ptr_store(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "atomic_ptr_swap" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_ptr_swap(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "atomic_ptr_cas" if arguments.len() == 3 => {
                source.push_str("__roze_atomic_ptr_cas(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push(')');
            }

            // Polymorphic Atomics
            "atomic_load" if arguments.len() == 1 => {
                source.push_str("__roze_atomic_load(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "atomic_store" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_store(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "atomic_swap" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_swap(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "atomic_cas" if arguments.len() == 3 => {
                source.push_str("__roze_atomic_cas(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push(')');
            }
            "atomic_add" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_add(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "atomic_sub" if arguments.len() == 2 => {
                source.push_str("__roze_atomic_sub(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }

            // Structured Concurrency (M1-T07)
            "nursery_new" | "task_group_new" if arguments.is_empty() => {
                source.push_str("__roze_nursery_new()");
            }
            "nursery_on_failure" | "task_group_on_failure" if arguments.is_empty() => {
                source.push_str("__roze_nursery_on_failure()");
            }
            "nursery_on_success" | "task_group_on_success" if arguments.is_empty() => {
                source.push_str("__roze_nursery_on_success()");
            }
            "nursery_with_timeout" | "task_group_with_timeout" if arguments.len() == 1 => {
                source.push_str("__roze_nursery_with_timeout(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "nursery_fork" | "task_group_fork" | "nursery_spawn" | "task_group_spawn" if arguments.len() == 2 => {
                source.push_str("__roze_nursery_fork(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "nursery_join" | "task_group_join" | "nursery_wait" | "task_group_wait" if arguments.len() == 1 => {
                source.push_str("__roze_nursery_join(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "nursery_cancel" | "task_group_cancel" if arguments.len() == 1 => {
                source.push_str("__roze_nursery_cancel(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "nursery_is_cancelled" | "task_group_is_cancelled" if arguments.len() == 1 => {
                source.push_str("__roze_nursery_is_cancelled(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "nursery_result" | "task_group_result" if arguments.len() == 1 => {
                source.push_str("__roze_nursery_result(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "nursery_token" | "task_group_token" if arguments.len() == 1 => {
                source.push_str("__roze_nursery_token(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "nursery_task_count" | "task_group_task_count" if arguments.len() == 1 => {
                source.push_str("__roze_nursery_task_count(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "nursery_completed_count" | "task_group_completed_count" if arguments.len() == 1 => {
                source.push_str("__roze_nursery_completed_count(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "subtask_get" if arguments.len() == 1 => {
                source.push_str("__roze_subtask_get(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "subtask_is_done" if arguments.len() == 1 => {
                source.push_str("__roze_subtask_is_done(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "subtask_is_cancelled" if arguments.len() == 1 => {
                source.push_str("__roze_subtask_is_cancelled(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "cancel_token_new" if arguments.is_empty() => {
                source.push_str("__roze_cancel_token_new()");
            }
            "cancel_token_cancel" if arguments.len() == 1 => {
                source.push_str("__roze_cancel_token_cancel(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "cancel_token_is_cancelled" if arguments.len() == 1 => {
                source.push_str("__roze_cancel_token_is_cancelled(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }

            // Direct memory intrinsics (MED-05)
            "alloc" if arguments.len() == 1 => {
                source.push_str("__roze_alloc(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "alloc_zeroed" if arguments.len() == 1 => {
                source.push_str("__roze_alloc_zeroed(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "realloc" if arguments.len() == 2 => {
                source.push_str("__roze_realloc(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "free" if arguments.len() == 1 => {
                source.push_str("__roze_free(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "ptr_read_u8" if arguments.len() == 1 => {
                source.push_str("__roze_ptr_read_u8(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "ptr_read" | "ptr_read_i64" if arguments.len() == 1 => {
                source.push_str("__roze_ptr_read(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "ptr_read_at" if arguments.len() == 2 => {
                source.push_str("__roze_ptr_read_at(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "ptr_read_u8_at" if arguments.len() == 2 => {
                source.push_str("__roze_ptr_read_u8_at(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "ptr_write_u8" if arguments.len() == 2 => {
                source.push_str("__roze_ptr_write_u8(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "ptr_write" | "ptr_write_i64" if arguments.len() == 2 => {
                source.push_str("__roze_ptr_write(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "ptr_write_at" if arguments.len() == 3 => {
                source.push_str("__roze_ptr_write_at(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push(')');
            }
            "ptr_write_u8_at" if arguments.len() == 3 => {
                source.push_str("__roze_ptr_write_u8_at(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push(')');
            }
            "ptr_add" | "ptr_offset" if arguments.len() == 2 => {
                source.push('(');
                self.generate_expression(source, &arguments[0])?;
                source.push_str(" + ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "ptr_diff" if arguments.len() == 2 => {
                source.push_str("((int) (");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(" - ");
                self.generate_expression(source, &arguments[1])?;
                source.push_str("))");
            }
            "ptr_to_int" if arguments.len() == 1 => {
                source.push_str("((int) ");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "int_to_ptr" if arguments.len() == 1 => {
                source.push_str("((long) ");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "ptr_is_null" if arguments.len() == 1 => {
                source.push('(');
                self.generate_expression(source, &arguments[0])?;
                source.push_str(" == 0L)");
            }
            "mem_copy" if arguments.len() == 3 => {
                source.push_str("__roze_mem_copy(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push(')');
            }
            "mem_set" if arguments.len() == 3 => {
                source.push_str("__roze_mem_set(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push(')');
            }

            "vga_clear" if arguments.is_empty() => {
                source.push_str("__roze_vga_clear()");
            }
            "vga_print" if arguments.len() == 1 => {
                source.push_str("__roze_vga_print(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "vga_write_char" if arguments.len() == 4 => {
                source.push_str("__roze_vga_write_char(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[3])?;
                source.push(')');
            }
            "vga_write_byte" if arguments.len() == 2 => {
                source.push_str("__roze_vga_write_byte(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vga_set_cursor" if arguments.len() == 2 => {
                source.push_str("__roze_vga_set_cursor(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "vga_set_color" if arguments.len() == 2 => {
                source.push_str("__roze_vga_set_color(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }

            // Kernel & Multiboot2 (ADV-02)
            "multiboot2_get_magic" if arguments.is_empty() => {
                source.push_str("__roze_multiboot2_get_magic()");
            }
            "multiboot2_get_info_ptr" if arguments.is_empty() => {
                source.push_str("__roze_multiboot2_get_info_ptr()");
            }
            "outb" if arguments.len() == 2 => {
                source.push_str("__roze_outb(");
                self.generate_expression(source, &arguments[0])?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1])?;
                source.push(')');
            }
            "inb" if arguments.len() == 1 => {
                source.push_str("__roze_inb(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "qemu_exit" if arguments.len() == 1 => {
                source.push_str("__roze_qemu_exit(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "serial_print" if arguments.len() == 1 => {
                source.push_str("__roze_serial_print(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }
            "serial_putchar" if arguments.len() == 1 => {
                source.push_str("__roze_serial_putchar(");
                self.generate_expression(source, &arguments[0])?;
                source.push(')');
            }

            "println" => {
                source.push_str("System.out.println(");
                self.generate_println_args(source, arguments)?;
                source.push(')');
            }
            _ => {
                source.push_str(&escape_java_identifier(name));
                source.push('(');
                for (i, arg) in arguments.iter().enumerate() {
                    if i > 0 {
                        source.push_str(", ");
                    }
                    self.generate_expression(source, arg)?;
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
    fn generate_int_arg(&self, source: &mut String, expr: &TypedExpression) -> Result<()> {
        if expr.type_ == Type::Int {
            self.generate_expression(source, expr)
        } else {
            source.push_str("((Integer)(Object)(");
            self.generate_expression(source, expr)?;
            source.push_str("))");
            Ok(())
        }
    }

    /// Same idea as `generate_int_arg`, but for a numeric argument to
    /// `abs`/`max`/`min` specifically, which have both an `int` and a
    /// `double` overload on the Java side. A statically Float-typed
    /// expression is left completely alone -- Java's own overload
    /// resolution picks `Math.abs(double)` etc. for it, and
    /// int-to-double widening handles a mixed int/float pair (e.g.
    /// `max(3, 7.5)`) without any cast needed on either argument.
    /// Anything else (Int, or an Unknown-typed value from an untyped
    /// list/map element) falls back to `generate_int_arg`'s existing
    /// Integer-unboxing behavior -- meaning a float value that ended up
    /// boxed into an untyped container and read back out would still
    /// crash here, a narrow, known limitation (fixing it needs a
    /// runtime `instanceof`-based dispatch, not attempted here).
    fn generate_numeric_arg(&self, source: &mut String, expr: &TypedExpression) -> Result<()> {
        if expr.type_ == Type::Float {
            self.generate_expression(source, expr)
        } else {
            self.generate_int_arg(source, expr)
        }
    }

    /// Generates an expression used as the receiver of a method call
    /// (e.g. the list in `list.add(x)`), wrapped in parens as a cheap,
    /// always-safe guard against needing to reason about whether the
    /// receiver expression needs them for any particular case.
    fn generate_receiver(&self, source: &mut String, expr: &TypedExpression) -> Result<()> {
        source.push('(');
        self.generate_expression(source, expr)?;
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
    fn generate_list_receiver(&self, source: &mut String, expr: &TypedExpression) -> Result<()> {
        if expr.type_ == Type::List {
            self.generate_receiver(source, expr)
        } else {
            source.push_str("((java.util.List)(");
            self.generate_expression(source, expr)?;
            source.push_str("))");
            Ok(())
        }
    }

    /// Like `generate_list_receiver`, for `Map`.
    fn generate_map_receiver(&self, source: &mut String, expr: &TypedExpression) -> Result<()> {
        if expr.type_ == Type::Map {
            self.generate_receiver(source, expr)
        } else {
            source.push_str("((java.util.Map)(");
            self.generate_expression(source, expr)?;
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
    fn generate_index_arg(&self, source: &mut String, expr: &TypedExpression) -> Result<()> {
        if expr.type_ == Type::Int {
            self.generate_expression(source, expr)
        } else {
            source.push_str("((Integer)(Object)(");
            self.generate_expression(source, expr)?;
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
        let typed = crate::semantic::check_and_lower(&program).expect("fixture should type-check");
        JavaSourceGenerator::new(typed, "Fixture".to_string())
            .generate()
            .expect("fixture should generate")
    }

    #[test]
    fn escapes_a_java_reserved_word() {
        assert_eq!(escape_java_identifier("assert"), "assert_");
        assert_eq!(escape_java_identifier("interface"), "interface_");
        assert_eq!(escape_java_identifier("synchronized"), "synchronized_");
        assert_eq!(escape_java_identifier("switch"), "switch_");
        assert_eq!(escape_java_identifier("true"), "true_");
        assert_eq!(escape_java_identifier("null"), "null_");
    }

    #[test]
    fn leaves_an_ordinary_identifier_alone() {
        assert_eq!(escape_java_identifier("total"), "total");
        assert_eq!(escape_java_identifier("assertion"), "assertion"); // a prefix match, not the word itself
        assert_eq!(escape_java_identifier("my_switch"), "my_switch");
    }

    #[test]
    fn does_not_escape_javas_contextual_keywords() {
        // var/yield/record/sealed/permits remain legal Java identifiers
        // in most positions -- escaping them would just be unnecessary
        // noise for names that virtually never actually need it.
        assert_eq!(escape_java_identifier("var"), "var");
        assert_eq!(escape_java_identifier("yield"), "yield");
        assert_eq!(escape_java_identifier("record"), "record");
    }

    #[test]
    fn function_named_after_a_java_keyword_compiles_and_is_escaped_consistently() {
        // The exact bug this guards against: a Roze function named
        // `assert` (a real Java keyword) used to fail to compile,
        // because codegen emitted Roze identifiers verbatim with no
        // check against Java's keyword list.
        let java = generate("func assert(x: bool) { println(x); } func main() { assert(true); }");
        assert!(java.contains("static void assert_("), "expected the declaration to be escaped:\n{}", java);
        assert!(java.contains("assert_(true)"), "expected the call site to be escaped to match:\n{}", java);
        assert!(!java.contains("void assert("), "must not emit the bare reserved word as a method name:\n{}", java);
    }

    #[test]
    fn variable_named_after_a_java_keyword_is_escaped_at_declaration_and_every_use() {
        let java = generate("func main() { let switch = \"hi\"; println(switch); switch = \"bye\"; println(switch); }");
        assert!(java.contains("String switch_ ="), "expected the declaration to be escaped:\n{}", java);
        assert!(java.contains("println(switch_)"), "expected reads to be escaped:\n{}", java);
        assert!(java.contains("switch_ = \"bye\""), "expected the reassignment target to be escaped:\n{}", java);
    }

    #[test]
    fn for_loop_variable_named_after_a_java_keyword_is_escaped() {
        let java = generate("func main() { for let case = 0; case < 3; case = case + 1 { println(case); } }");
        assert!(java.contains("int case_ = 0"), "expected the loop variable's declaration to be escaped:\n{}", java);
        assert!(java.contains("case_ < 3"), "expected the condition to be escaped:\n{}", java);
        assert!(java.contains("case_ = (case_ + 1)"), "expected the update clause to be escaped:\n{}", java);
    }

    #[test]
    fn parameter_named_after_a_java_keyword_is_escaped() {
        let java = generate("func f(interface: int) -> int { return interface; } func main() { }");
        assert!(java.contains("int interface_)"), "expected the parameter declaration to be escaped:\n{}", java);
        assert!(java.contains("return interface_;"), "expected the parameter's use in the body to be escaped:\n{}", java);
    }

    // ---- Classes ----

    #[test]
    fn class_becomes_a_real_separate_java_class() {
        let java = generate("class Point { x: int, y: int } func main() { let p = new Point(1, 2); }");
        assert!(java.contains("class Point {"), "expected a real Java class:\n{}", java);
        assert!(java.contains("public int x;"), "expected a public field:\n{}", java);
        assert!(java.contains("public int y;"), "expected a public field:\n{}", java);
        assert!(java.contains("public Point(int x, int y)"), "expected an all-fields constructor:\n{}", java);
        assert!(java.contains("this.x = x;"), "expected the constructor to assign fields:\n{}", java);
        assert!(java.contains("this.y = y;"), "expected the constructor to assign fields:\n{}", java);
        assert!(java.contains("new Point(1, 2)"), "expected construction to compile to 'new':\n{}", java);
    }

    #[test]
    fn field_access_and_assignment_compile_correctly() {
        let java = generate("class Point { x: int } func main() { let p = new Point(1); println(p.x); p.x = 5; }");
        assert!(java.contains("println(p.x)"), "expected field read to compile to '.field':\n{}", java);
        assert!(java.contains("p.x = 5;"), "expected field assignment to compile to '.field = value':\n{}", java);
    }

    #[test]
    fn class_name_and_field_name_reserved_word_collisions_are_escaped_everywhere() {
        // The exact bug found during development: a class named
        // `interface` (a Java keyword) correctly escaped at its `new
        // interface_(...)` call site, but NOT in the *type* position
        // right next to it (a variable's declared type, a parameter, a
        // return type) -- because Type::to_java() itself has no
        // business depending on codegen's reserved-word list, every
        // site that emits a declared type needed to route through a
        // separate escaping-aware helper instead.
        let java = generate(
            "class interface { assert: int } \
             func make() -> interface { return new interface(1); } \
             func main() { let obj = make(); println(obj.assert); }"
        );
        assert!(java.contains("class interface_ {"), "expected the class declaration itself to be escaped:\n{}", java);
        assert!(java.contains("public int assert_;"), "expected the field declaration to be escaped:\n{}", java);
        assert!(java.contains("public interface_(int assert_)"), "expected the constructor name and parameter to be escaped:\n{}", java);
        assert!(java.contains("interface_ make()"), "expected the return type position to be escaped, not just the constructor call:\n{}", java);
        assert!(java.contains("new interface_(1)"), "expected the constructor call to be escaped:\n{}", java);
        assert!(java.contains("interface_ obj = make();"), "expected the variable's declared type to be escaped:\n{}", java);
        assert!(java.contains("obj.assert_"), "expected the field access to be escaped:\n{}", java);
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

    #[test]
    fn http_server_handle_is_cast_at_each_use_site() {
        // http_server_start's declared type is Unknown/Object (an opaque
        // handle -- there's no ServerSocket variant in the Type enum),
        // but the helper methods it's passed to declare a
        // java.net.ServerSocket parameter. Without an explicit cast at
        // each call site, javac rejects passing an Object where
        // ServerSocket is declared.
        let java = generate("func main() { let s = http_server_start(8080); http_server_accept(s); http_server_stop(s); }");
        assert!(java.contains("(java.net.ServerSocket)("), "expected an explicit ServerSocket cast:\n{}", java);
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
