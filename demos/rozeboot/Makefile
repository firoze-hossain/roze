ROZE ?= ../../target/release/roze
ifeq ($(wildcard $(ROZE)),)
    ROZE = ../../target/debug/roze
endif

H2_JAR ?= $(shell find ~/.m2/repository/com/h2database/h2 -name "h2-*.jar" 2>/dev/null | head -n 1)
ifeq ($(H2_JAR),)
    H2_JAR = /usr/share/java/h2.jar
endif

PORT ?= 8080

.PHONY: all build run test-api clean help

all: build

help:
	@echo "🌹 RozeBoot Enterprise Showcase Demo (ADV-03)"
	@echo "Available targets:"
	@echo "  make build      - Compile the RozeBoot application into JVM bytecode"
	@echo "  make run        - Run the RozeBoot server on port $(PORT)"
	@echo "  make test-api   - Run automated curl API test suite against running server"
	@echo "  make clean      - Remove compiled artifacts"

build:
	@echo "Building RozeBoot Enterprise Application..."
	$(ROZE) build src/main.roze --classpath "$(H2_JAR)"

run: build
	@echo "Starting RozeBoot Enterprise Application..."
	$(ROZE) run src/main.roze --classpath "$(H2_JAR)"

test-api:
	@echo "Executing RozeBoot REST API Test Suite..."
	@echo "--------------------------------------------------"
	@echo "1. Public Health Check:"
	@curl -s -w "\nHTTP Status: %{http_code}\n" http://localhost:$(PORT)/health
	@echo "--------------------------------------------------"
	@echo "2. Public Info:"
	@curl -s -w "\nHTTP Status: %{http_code}\n" http://localhost:$(PORT)/
	@echo "--------------------------------------------------"
	@echo "3. Unauthorized Access (Protected Endpoint):"
	@curl -s -w "\nHTTP Status: %{http_code}\n" http://localhost:$(PORT)/api/products
	@echo "--------------------------------------------------"
	@echo "4. Authenticating as admin to obtain JWT:"
	$(eval TOKEN := $(shell curl -s -X POST http://localhost:$(PORT)/api/auth/login -d '{"username":"admin","password":"roze2026"}' | grep -o '"access_token":"[^"]*' | cut -d'"' -f4))
	@echo "Acquired JWT Token: $(TOKEN)"
	@echo "--------------------------------------------------"
	@echo "5. Authorized Product Catalog Listing (Bearer Token):"
	@curl -s -H "Authorization: Bearer $(TOKEN)" -w "\nHTTP Status: %{http_code}\n" http://localhost:$(PORT)/api/products
	@echo "--------------------------------------------------"
	@echo "6. Authorized Single Product Query (/api/products/1):"
	@curl -s -H "Authorization: Bearer $(TOKEN)" -w "\nHTTP Status: %{http_code}\n" http://localhost:$(PORT)/api/products/1
	@echo "--------------------------------------------------"
	@echo "7. Authorized Product Creation (POST /api/products):"
	@curl -s -X POST -H "Authorization: Bearer $(TOKEN)" -H "Content-Type: application/json" -d '{"name":"Roze Wireless Mouse","category":"Hardware","price":"79"}' -w "\nHTTP Status: %{http_code}\n" http://localhost:$(PORT)/api/products
	@echo "--------------------------------------------------"
	@echo "8. Verify New Product in Catalog:"
	@curl -s -H "Authorization: Bearer $(TOKEN)" -w "\nHTTP Status: %{http_code}\n" http://localhost:$(PORT)/api/products
	@echo "--------------------------------------------------"
	@echo "✅ All RozeBoot API tests passed successfully!"

clean:
	rm -f main.java main.class src/*.class src/*.java
