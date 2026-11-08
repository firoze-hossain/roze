# roze-orm

High-throughput asynchronous ORM, HikariCP-equivalent connection pool, and type-safe SQL query builder for Roze.

## Features

- **Entity Metadata & Annotations**: `@Entity`, `@Table`, `@Id`, `@Column`, `@Version` declarative schema definitions
- **Connection Pool**: High-performance connection pooling with active leak detection and telemetry
- **Fluent QueryBuilder**: Type-safe AST SQL builder supporting `SELECT`, `JOIN`, `WHERE`, `ORDER BY`, `GROUP BY`, `INSERT`, `UPDATE`, `DELETE`
- **Parameterized Queries**: Automatic anti-SQL injection parameter binding with `?` placeholders
- **Transactions & Isolation**: ACID transaction lifecycle, savepoints, automatic rollback, and optimistic locking
- **Schema Migrations**: Versioned DDL migrations with cryptographic checksum verification and rollback
- **Repository Pattern**: Spring Data JPA-style repository interfaces and ActiveRecord pattern

## Installation

Add to your `bsr.mfz`:

```toml
[dependencies]
roze-orm = "^1.0.0"
```

## Usage

```roze
import "roze-orm";

func main() {
    let qb = sql_select_from("users");
    sql_col(qb, "id");
    sql_col(qb, "username");
    sql_col(qb, "email");
    sql_where_eq(qb, "active", "1");
    sql_order(qb, "created_at", "DESC");
    sql_limit(qb, 10);

    let sql = sql_to_string(qb);
    println("Generated SQL: " + sql);
}
```
