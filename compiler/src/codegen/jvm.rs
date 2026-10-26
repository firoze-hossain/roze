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
    private final Object[] buffer;
    private final int capacity;
    private int head = 0;
    private int tail = 0;
    private int count = 0;
    private boolean closed = false;
    private final java.util.concurrent.locks.ReentrantLock lock = new java.util.concurrent.locks.ReentrantLock();
    private final java.util.concurrent.locks.Condition notEmpty = lock.newCondition();
    private final java.util.concurrent.locks.Condition notFull = lock.newCondition();

    public RozeChannel(int capacity) {
        if (capacity <= 0) capacity = 64;
        this.capacity = capacity;
        this.buffer = new Object[capacity];
    }

    public boolean send(Object val) {
        lock.lock();
        try {
            while (count == capacity && !closed) {
                notFull.await();
            }
            if (closed) return false;
            buffer[tail] = val;
            tail = (tail + 1) % capacity;
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

    public Object recv() {
        lock.lock();
        try {
            while (count == 0 && !closed) {
                notEmpty.await();
            }
            if (count == 0 && closed) return null;
            Object val = buffer[head];
            buffer[head] = null;
            head = (head + 1) % capacity;
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
}
"#;

const CONCURRENCY_HELPER_METHODS: &str = r#"
    private static RozeThreadHandle __roze_thread_spawn(Object fnObj) {
        RozeFn fn = (RozeFn) fnObj;
        return new RozeThreadHandle(() -> fn.apply());
    }

    private static Object __roze_thread_join(Object handleObj) {
        if (handleObj instanceof RozeThreadHandle) {
            return ((RozeThreadHandle) handleObj).join();
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
        "channel_new" => Some("RozeChannel"),
        "channel_send" | "send" => Some("boolean"),
        "channel_recv" | "recv" => Some(OBJECT_TYPE),
        "channel_close" => Some("void"),
        "channel_is_closed" => Some("boolean"),
        "channel_len" => Some("int"),

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
        Type::Float => format!("((Number) {}).doubleValue()", expr_str),
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
        source.push_str("}\n");

        source.push_str(OPTION_RESULT_HELPER_CLASSES);
        source.push_str(CONCURRENCY_HELPER_CLASSES);

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

                source.push_str(&format!("{}Object {} = ", indent_str, target_var));
                self.generate_expression(source, &expr)?;
                source.push_str(";\n");

                let mut is_first = true;
                for arm in arms {
                    source.push_str(&indent_str);
                    if !is_first {
                        source.push_str("else ");
                    }
                    is_first = false;

                    match &arm.pattern {
                        TypedPattern::Wildcard { .. } => {
                            source.push_str("{\n");
                            self.generate_statement(source, &arm.body, indent + 1, return_type, is_closure)?;
                            source.push_str(&format!("{}}}\n", indent_str));
                        }
                        TypedPattern::Variable { name, type_, .. } => {
                            source.push_str("{\n");
                            source.push_str(&format!(
                                "{}    {} {} = ({}) {};\n",
                                indent_str, java_type_name(type_), escape_java_identifier(name), java_type_name(type_), target_var
                            ));
                            self.generate_statement(source, &arm.body, indent + 1, return_type, is_closure)?;
                            source.push_str(&format!("{}}}\n", indent_str));
                        }
                        TypedPattern::Literal { expr: lit, .. } => {
                            source.push_str(&format!("if (java.util.Objects.equals({}, ", target_var));
                            self.generate_expression(source, lit)?;
                            source.push_str(")) {\n");
                            self.generate_statement(source, &arm.body, indent + 1, return_type, is_closure)?;
                            source.push_str(&format!("{}}}\n", indent_str));
                        }
                        TypedPattern::Variant { enum_name, variant, bindings, .. } => {
                            let variant_class = format!("{}.{}", escape_java_identifier(enum_name), escape_java_identifier(variant));
                            source.push_str(&format!("if ({} instanceof {}) {{\n", target_var, variant_class));

                            if !bindings.is_empty() {
                                let arm_var = format!("__arm_val_{}", self.try_counter.get());
                                self.try_counter.set(self.try_counter.get() + 1);
                                source.push_str(&format!(
                                    "{}    {} {} = ({}) {};\n",
                                    indent_str, variant_class, arm_var, variant_class, target_var
                                ));
                                for (i, (b_name, b_type)) in bindings.iter().enumerate() {
                                    if b_name != "_" {
                                        source.push_str(&format!(
                                            "{}    {} {} = {}._{};\n",
                                            indent_str, java_type_name(b_type), escape_java_identifier(b_name), arm_var, i
                                        ));
                                    }
                                }
                            }
                            self.generate_statement(source, &arm.body, indent + 1, return_type, is_closure)?;
                            source.push_str(&format!("{}}}\n", indent_str));
                        }
                    }
                }

                let has_catchall = arms.iter().any(|arm| matches!(arm.pattern, TypedPattern::Wildcard { .. } | TypedPattern::Variable { .. }));
                if !has_catchall && !arms.is_empty() {
                    source.push_str(&format!("{}else {{\n", indent_str));
                    source.push_str(&format!("{}    throw new RuntimeException(\"Pattern matching failed: no arm matched \" + {});\n", indent_str, target_var));
                    source.push_str(&format!("{}}}\n", indent_str));
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
            return self.generate_expression(source, &arguments[0]);
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
            TypedExpressionKind::Number(value) => {
                if !value.contains('.') && (value.parse::<i64>().is_ok() && value.parse::<i32>().is_err()) {
                    source.push_str(value);
                    source.push('L');
                } else {
                    source.push_str(value);
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
                if (function == "list_get" || function == "map_get" || function == "channel_recv" || function == "recv" || function == "thread_join") && expr.type_ != Type::Unknown {
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
                let is_primitive = |t: &Type| matches!(t, Type::Int | Type::Bool);

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
            "channel_new" => {
                source.push_str("__roze_channel_new(");
                if arguments.len() == 1 {
                    self.generate_expression(source, &arguments[0])?;
                } else {
                    source.push_str("64");
                }
                source.push(')');
            }
            "channel_send" | "send" if arguments.len() == 2 => {
                source.push_str("__roze_channel_send(");
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
