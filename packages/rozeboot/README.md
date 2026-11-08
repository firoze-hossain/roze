# rozeboot

Spring-Boot-style Enterprise IoC, REST microservices, OpenAPI 3.1 & cloud observability framework for Roze.

## Features

- **IoC Container & Annotations**: `@Service`, `@Autowired`, `@Component`, `@Controller`, `@GetMapping`, `@PostMapping`
- **Dynamic REST Routing**: Path parameter extraction (`:id`), query params, header injection, and JSON body parsing
- **Constraint Validation**: Declarative validation rules (`rule_not_null`, `rule_email`, `rule_min`, `rule_max`, `rule_pattern`)
- **Problem Details**: RFC 7807 compliant structured error payloads
- **Security & JWT**: HMAC-SHA256 JWT generation, verification, and path-excluded security guard middleware
- **OpenAPI 3.1 & Swagger**: Automatic API documentation generation and Swagger UI serving
- **Metrics & Observability**: Prometheus metrics exporter and runtime telemetry

## Installation

Add to your `bsr.mfz`:

```toml
[dependencies]
rozeboot = "^1.0.0"
```

## Usage

```roze
import "rozeboot";

func main() {
    let app = rozeboot2_new("OrderService", 8080);

    let ctrl = controller_binding_new("OrderController", "/api/orders");
    controller_bind_route(ctrl, "GET", "", "getAllOrders");
    rozeboot2_register_controller(app, ctrl);

    let booted = rozeboot2_boot(app);
    if booted {
        println("🚀 RozeBoot service successfully started on port 8080");
    }
}
```
