# 🌹 RozeBoot - Enterprise Spring-Boot-Style Framework for Roze (ADV-03)

**RozeBoot** is a Spring-Boot-inspired enterprise framework designed to build large-scale cloud microservices, APIs, and enterprise web applications in Roze.

It combines declarative REST routing, connection pooling (HikariCP / fallback high-performance thread-safe queue pool), standard HMAC-SHA256 JWT security, and a fluent SQL ORM query builder.

---

## Architecture & Features

```
                   ┌─────────────────────────────────────────┐
                   │          HTTP Client (cURL / UI)        │
                   └────────────────────┬────────────────────┘
                                        │
                                        ▼
                   ┌─────────────────────────────────────────┐
                   │         RozeBoot App Pipeline           │
                   ├─────────────────────────────────────────┤
                   │  1. CORS Middleware                     │
                   │  2. Structured Request Logger           │
                   │  3. JWT Security Guard & Token Decoder  │
                   └────────────────────┬────────────────────┘
                                        │
                         ┌──────────────┴──────────────┐
                         ▼                             ▼
             ┌───────────────────────┐     ┌───────────────────────┐
             │    AuthController     │     │   ProductsController  │
             │   /api/auth/login     │     │     /api/products     │
             └───────────────────────┘     └───────────┬───────────┘
                                                       │
                                                       ▼
                                           ┌───────────────────────┐
                                           │  Fluent QueryBuilder  │
                                           │  (SELECT/INSERT/etc)  │
                                           └───────────┬───────────┘
                                                       │
                                                       ▼
                                           ┌───────────────────────┐
                                           │ HikariCP / JDBC Pool  │
                                           │ (H2 / SQLite/Postgres)│
                                           └───────────────────────┘
```

### 1. Declarative REST Controllers
Define modular controllers with URL prefixes and HTTP methods:
```roze
let ctrl = controller_new("ProductsController", "/api/products");

// GET /api/products/:id
controller_get(ctrl, "/:id", |req: map| {
    let id = req_param(req, "id");
    let item = query_table("products").query_where("id", "=", id).query_first(app.datasource);
    if map_size(item) == 0 {
        resp_not_found(req, "Not found");
        return;
    }
    resp_ok(req, item);
});
```

### 2. Enterprise Connection Pooling (`HikariCP`)
Full support for HikariCP when `HikariCP.jar` is present on the classpath, with automatic fallback to a high-performance thread-safe `LinkedBlockingQueue` JDBC connection pool with leak protection and real-time statistics:
```roze
rozeboot_with_datasource(app, "jdbc:h2:mem:rozedb;DB_CLOSE_DELAY=-1", 10);
let stats = datasource_stats(app.datasource);
println(map_get(stats, "active"));
println(map_get(stats, "max_pool_size"));
```

### 3. JWT Security Guard
Zero-dependency cryptographic HMAC-SHA256 JWT token generation and authentication guard filter:
```roze
// Generate 1-hour valid token
let token = jwt_generate(username, "ROLE_ADMIN", 3600, jwt_secret);

// Protect all routes except public ones
let public_paths = list_new();
list_push(public_paths, "/health");
list_push(public_paths, "/api/auth/login");
rozeboot_with_jwt(app, jwt_secret, public_paths);
```
Unauthenticated requests to protected endpoints automatically receive `401 Unauthorized` with JSON error payload.

### 4. Fluent Type-Safe SQL ORM Query Builder
Write SQL safely and composably without raw string concatenation:
```roze
// SELECT id, name, price FROM products WHERE category = 'Hardware' ORDER BY price DESC LIMIT 10
let qb = query_table("products");
query_select(qb, cols);
query_where(qb, "category", "=", "Hardware");
query_order_by(qb, "price", "DESC");
query_limit(qb, 10);

let results = query_get(qb, app.datasource);
```

---

## Quickstart

### Prerequisites
- JDK 11+ (`javac` and `java` on PATH)
- Roze Compiler (`cargo build --release`)

### Build & Run
```bash
# In demos/rozeboot directory:
make build
make run
```

### Testing the REST API
In a separate terminal:
```bash
make test-api
```

### Manual cURL Commands
```bash
# 1. Health check (Public)
curl -i http://localhost:8080/health

# 2. Try accessing protected resource (Returns 401 Unauthorized)
curl -i http://localhost:8080/api/products

# 3. Authenticate and obtain JWT
TOKEN=$(curl -s -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"roze2026"}' | jq -r '.access_token')

# 4. Access protected catalog with Bearer token
curl -i -H "Authorization: Bearer $TOKEN" http://localhost:8080/api/products

# 5. Add a new product via POST
curl -i -X POST http://localhost:8080/api/products \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"Roze Studio Display","category":"Hardware","price":"999"}'
```
