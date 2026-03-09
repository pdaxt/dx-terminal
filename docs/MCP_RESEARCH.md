# MCP Deep Research: Composition, Architecture & Implementation Patterns

> Research Date: 2026-03-02
> Focus: How to build composable MCP systems where servers can spawn/manage other servers

## Executive Summary

The Model Context Protocol (MCP) supports multiple composition patterns for building modular AI applications. This research covers the rmcp crate (v0.15.0), JSON-RPC 2.0 protocol, existing gateway implementations, and practical patterns for dynamic MCP composition.

**Key Finding:** MCPs can be composed through 4 main patterns:
1. **In-process routers** (rmcp ToolRouter combining)
2. **Child process spawning** (TokioChildProcess)
3. **HTTP proxy gateways** (MetaMCP, Microsoft MCP Gateway)
4. **Dynamic discovery** (on-demand MCP loading)

---

## 1. rmcp Crate Architecture (v0.15.0)

### Core Components

**Transport Layer:**
- `stdio`: Uses tokio stdin/stdout for JSON-RPC 2.0 messages (newline-delimited)
- `TokioChildProcess`: Spawns child processes with piped stdio for MCP clients
- `StreamableHttpClient/Server`: HTTP SSE transport for web-based MCPs
- `AsyncRwTransport`: Generic wrapper for any async read/write byte stream

**Service Layer:**
- Implements JSON-RPC 2.0 protocol over transport
- `ServiceRole` trait distinguishes `RoleServer` vs `RoleClient`
- Bidirectional: both sides can send requests and notifications
- Session-aware messaging with unique request IDs

**Handler Layer:**
- `ServerHandler` trait: implement `call_tool()`, `list_tools()`, `get_info()`
- `#[tool]` macro: auto-generates JSON schema from Rust function signature
- `#[tool_router]` macro: creates router from all `#[tool]` annotated functions
- `#[tool_handler]` macro: implements `ServerHandler` using the router

### MCP Server Pattern in Rust

```rust
#[derive(Clone)]
pub struct MyMCPService {
    app: Arc<SomeState>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl MyMCPService {
    pub fn new(app: Arc<SomeState>) -> Self {
        Self {
            app,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Does something useful")]
    async fn my_tool(
        &self,
        Parameters(req): Parameters<MyRequest>
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = format!("Processing: {:?}", req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }
}

#[tool_handler]
impl ServerHandler for MyMCPService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("My MCP server description".into()),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            ..Default::default()
        }
    }
}

pub async fn run_mcp_server(app: Arc<SomeState>) -> anyhow::Result<()> {
    let server = MyMCPService::new(app);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
```

**Key Insight:** The `#[tool_router]` macro generates a `tool_router()` function that creates a router containing all tools. Multiple routers can be combined with `+` operator for composition.

---

## 2. MCP Protocol Specification (JSON-RPC 2.0)

### Message Format

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "my_tool",
    "arguments": {"key": "value"}
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {"type": "text", "text": "result"}
    ]
  }
}
```

**Notification (no response expected):**
```json
{
  "jsonrpc": "2.0",
  "method": "notifications/cancelled",
  "params": {}
}
```

### Stdio Transport Details

- **Format:** Newline-delimited JSON (`\n` separator)
- **Constraint:** JSON must NOT contain embedded newlines
- **Channels:**
  - Stdin: receives requests from client
  - Stdout: sends responses to client
  - Stderr: for logs (not part of protocol)

### Initialization Handshake

1. Client sends `initialize` request with client capabilities
2. Server responds with server capabilities + info
3. Client sends `initialized` notification
4. Session is now active, tools can be called

**Source:** [MCP Transports - Model Context Protocol](https://modelcontextprotocol.io/legacy/concepts/transports)

---

## 3. Spawning Child MCP Servers

### Using TokioChildProcess (rmcp v0.15.0)

```rust
use rmcp::transport::TokioChildProcess;
use tokio::process::Command;

// Spawn an MCP server as child process
let child = TokioChildProcess::new(
    Command::new("node")
        .arg("path/to/mcp-server.js")
)?;

// Create a client that talks to the child via stdio
let client = ().serve(child).await?;

// Now you can call tools on the child MCP
let tools = client.peer().list_tools(Default::default()).await?;
let result = client.peer().call_tool("tool_name", args).await?;
```

### Graceful Shutdown

```rust
// Closes transport, waits up to 3 seconds, then kills if needed
child.graceful_shutdown().await?;
```

### Builder Pattern for Advanced Stdio Control

```rust
let (child, stderr) = TokioChildProcess::builder(Command::new("mcp-server"))
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())  // Capture stderr separately
    .spawn()?;
```

**Source:** rmcp crate `/src/transport/child_process.rs`

---

## 4. MCP Gateway Patterns (State of the Art 2026)

### Microsoft MCP Gateway

**Architecture:**
- **Dual-Plane Design:**
  - Data Plane: runtime request routing with session affinity
  - Control Plane: lifecycle management via RESTful APIs
- **Kubernetes-Native:**
  - StatefulSets with headless services
  - Distributed session store for multi-replica gateways
  - CRDs for declarative MCP server definitions

**Key Features:**
- Tool registration with dynamic routing
- Session-aware stateful routing (session_id → same pod)
- Auto-scaling based on load
- Graceful termination and rolling updates

**Use Case:** Enterprise-scale MCP deployment in production K8s clusters

**Source:** [Microsoft mcp-gateway GitHub](https://github.com/microsoft/mcp-gateway)

---

### MetaMCP

**Architecture:**
- Single Docker container
- Acts as Proxy + Aggregator + Middleware + Gateway
- Namespace-based server grouping
- Public endpoints with authentication (SSE/Streamable HTTP)

**Key Features:**
- Dynamic server aggregation at runtime
- Tool filtering middleware (e.g., "filter-inactive-tools")
- Observability and security middleware hooks
- Remix tools from multiple backend servers

**Use Case:** Lightweight proxy for combining MCPs without Kubernetes overhead

**Source:** [MetaMCP GitHub](https://github.com/metatool-ai/metamcp)

---

### Agentic Community MCP Gateway & Registry

**Architecture:**
- Virtual MCP servers (logical grouping of tools)
- Tool aliasing and version pinning
- Per-tool scope-based access control
- OAuth integration (Keycloak/Entra ID)

**Key Features:**
- Dynamic tool discovery from registry
- Session multiplexing (one client → many backends)
- Lua scripting for custom routing logic
- Enterprise governance and audit trails
- Cached aggregation for `list_tools()` operations

**Use Case:** Governed tool access for AI agents in enterprise environments

**Source:** [Agentic Community mcp-gateway-registry](https://github.com/agentic-community/mcp-gateway-registry)

---

## 5. FastMCP Server Composition (Python)

### mount() - Live Link Pattern

```python
from fastmcp import FastMCP

main = FastMCP("main")
subserver = FastMCP("sub")

@subserver.tool()
def sub_tool():
    return "from sub"

# Live link: requests delegated at runtime
main.mount("prefix/", subserver)
```

Tools from `subserver` are now accessible as `prefix/sub_tool`.

### import_server() - Static Copy Pattern

```python
# Copies tools/resources with prefix (one-time)
main.import_server(subserver, prefix="sub_")
```

Tools are copied into main server at initialization time.

### Benefits of Composition

- **Modularity:** Each service developed and tested independently
- **Reusability:** Build utility servers once, mount everywhere
- **Team Collaboration:** Different teams own different servers
- **Clean Naming:** Prefixes prevent tool name collisions

**Source:** [FastMCP Server Composition](https://gofastmcp.com/servers/composition)

---

## 6. MCP Composition Patterns

### Pattern 1: Proxy/Gateway (Runtime Routing)

```
Client → Gateway MCP → routes to Child MCP A, B, C based on tool name
                    → aggregates list_tools() from all children
```

**Implementation Sketch:**

```rust
struct GatewayMCP {
    children: HashMap<String, Arc<RmcpClient>>,
}

impl GatewayMCP {
    async fn call_tool(&self, name: &str, args: Value) -> Result<Value> {
        let (mcp_name, tool_name) = parse_prefixed_tool(name)?;
        let child = self.children.get(mcp_name)?;
        child.peer().call_tool(tool_name, args).await
    }

    async fn list_tools(&self) -> Vec<Tool> {
        let mut all_tools = Vec::new();
        for (prefix, child) in &self.children {
            let tools = child.peer().list_tools(Default::default()).await?;
            all_tools.extend(
                tools.tools.into_iter()
                    .map(|t| prefix_tool(prefix, t))
            );
        }
        all_tools
    }
}
```

---

### Pattern 2: Embedded Servers (In-Process)

```
Main MCP creates sub-services directly (no child processes)
```

**rmcp Example:**

```rust
#[tool_router(router = router_a, vis = "pub")]
impl ModuleA {
    #[tool] async fn tool_a(&self) -> Result<CallToolResult> { ... }
}

#[tool_router(router = router_b, vis = "pub")]
impl ModuleB {
    #[tool] async fn tool_b(&self) -> Result<CallToolResult> { ... }
}

impl MainService {
    fn new() -> Self {
        Self {
            // Combine routers with + operator
            tool_router: router_a() + router_b(),
        }
    }
}
```

**Pros:** Fast (no IPC), simple
**Cons:** Monolithic binary, no isolation

---

### Pattern 3: Dynamic MCP Discovery

```
Agent: "I need database tools"
→ Gateway searches registry
→ Gateway spawns postgres_mcp on-demand
→ Tools become available
→ Agent calls postgres.query()
```

**Implementation Flow:**

1. Agent calls `mcp_discover(capability="database")`
2. Gateway searches registry: finds `postgres_mcp`, `sqlite_mcp`
3. Gateway spawns child:
   ```rust
   let child = TokioChildProcess::new(
       Command::new("npx")
           .arg("-y")
           .arg("@modelcontextprotocol/server-postgres")
   )?;
   ```
4. Gateway initializes client via JSON-RPC handshake
5. Gateway caches running MCP for reuse
6. Gateway returns: "Added 15 tools from postgres_mcp"

**Source:** [Dynamic MCP - Docker AI](https://docs.docker.com/ai/mcp-catalog-and-toolkit/dynamic-mcp/)

---

### Pattern 4: Namespace/Virtual Servers

```
Single physical gateway exposes multiple "virtual MCPs"
Each virtual MCP = curated subset of tools from backends
```

**Config Example:**

```json
{
  "virtual_servers": {
    "dev": {
      "mcps": ["postgres", "playwright", "filesystem"],
      "tool_prefix": true
    },
    "prod": {
      "mcps": ["postgres"],
      "tool_prefix": false,
      "forbidden_tools": ["filesystem.*"]
    }
  }
}
```

Client connects to `/virtual/dev` or `/virtual/prod` and sees different tool sets.

---

## 7. Building the Lego Block System

### Core Primitive: MCP Descriptor

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MCPDescriptor {
    name: String,
    command: Vec<String>,  // ["node", "server.js"] or ["npx", "-y", "pkg"]
    capabilities: Vec<String>,  // ["database", "sql"]
    auto_start: bool,
    startup_timeout_ms: u64,
}
```

### Registry + Lifecycle Manager

```rust
struct MCPRegistry {
    descriptors: HashMap<String, MCPDescriptor>,
    running: HashMap<String, Arc<RunningMCP>>,
}

struct RunningMCP {
    child: TokioChildProcess,
    client: Arc<RmcpClient>,
    tools: Vec<Tool>,
    last_used: Instant,
}

impl MCPRegistry {
    async fn ensure_running(&mut self, name: &str) -> Result<Arc<RunningMCP>> {
        // Return cached if already running
        if let Some(mcp) = self.running.get(name) {
            return Ok(Arc::clone(mcp));
        }

        // Spawn new child process
        let desc = self.descriptors.get(name)?;
        let child = TokioChildProcess::new(
            Command::new(&desc.command[0])
                .args(&desc.command[1..])
        )?;

        // Initialize MCP client
        let client = ().serve(child).await?;
        let tools = client.peer().list_tools(Default::default()).await?;

        // Cache and return
        let mcp = Arc::new(RunningMCP {
            child,
            client,
            tools: tools.tools,
            last_used: Instant::now(),
        });
        self.running.insert(name.clone(), Arc::clone(&mcp));
        Ok(mcp)
    }

    async fn gc_idle(&mut self, max_idle: Duration) {
        // Shutdown MCPs that haven't been used recently
        self.running.retain(|_, mcp| {
            mcp.last_used.elapsed() < max_idle
        });
    }
}
```

### Composable Gateway MCP

```rust
struct GatewayMCP {
    registry: Arc<Mutex<MCPRegistry>>,
}

#[tool_router]
impl GatewayMCP {
    #[tool(description = "Discover and load MCPs by capability")]
    async fn mcp_discover(
        &self,
        Parameters(req): Parameters<DiscoverRequest>
    ) -> Result<CallToolResult> {
        let mut reg = self.registry.lock().await;
        let matches = reg.search_by_capability(&req.capability);

        let mut loaded = Vec::new();
        for name in matches {
            reg.ensure_running(&name).await?;
            loaded.push(name);
        }

        let result = format!("Loaded MCPs: {}", loaded.join(", "));
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Call any loaded MCP tool")]
    async fn mcp_call(
        &self,
        Parameters(req): Parameters<CallRequest>
    ) -> Result<CallToolResult> {
        let reg = self.registry.lock().await;
        let mcp = reg.ensure_running(&req.mcp_name).await?;

        let result = mcp.client.peer().call_tool(
            CallToolRequestParam {
                name: req.tool_name,
                arguments: req.arguments,
            },
            Default::default()
        ).await?;

        Ok(result)
    }

    #[tool(description = "List all available MCPs in registry")]
    async fn mcp_list_available(&self) -> Result<CallToolResult> {
        let reg = self.registry.lock().await;
        let list = reg.descriptors.keys()
            .map(|name| format!("- {}", name))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(CallToolResult::success(vec![Content::text(list)]))
    }
}
```

---

## 8. Configuration-Driven Composition

```json
{
  "mcps": {
    "postgres": {
      "command": ["npx", "-y", "@modelcontextprotocol/server-postgres"],
      "env": {
        "DATABASE_URL": "postgresql://localhost/mydb"
      },
      "capabilities": ["database", "sql"],
      "auto_start": true
    },
    "playwright": {
      "command": ["npx", "-y", "@modelcontextprotocol/server-playwright"],
      "capabilities": ["browser", "testing", "vision"],
      "auto_start": false
    },
    "filesystem": {
      "command": ["npx", "-y", "@modelcontextprotocol/server-filesystem"],
      "args": ["/allowed/path"],
      "capabilities": ["filesystem"],
      "auto_start": false
    }
  },
  "virtual_servers": {
    "dev": {
      "mcps": ["postgres", "playwright", "filesystem"],
      "tool_prefix": true
    },
    "prod": {
      "mcps": ["postgres"],
      "tool_prefix": false
    }
  }
}
```

---

## 9. Design Principles for Composable MCPs

### Core Principles

1. **Single Responsibility:** Each MCP does ONE thing well
2. **Stateless Tools:** Tools should be idempotent where possible
3. **Clear Contracts:** JSON schema enforces strict input/output types
4. **Namespacing:** Use prefixes to avoid tool name collisions
5. **Lazy Loading:** Don't spawn MCPs until first use
6. **Graceful Degradation:** Gateway continues if child MCP fails

### Performance Considerations

- **Startup Cost:** Spawning child process + initialization ~100-500ms
- **Connection Pooling:** Keep frequently-used MCPs running
- **Batch Operations:** Combine multiple tool calls when possible
- **Caching:** Cache `list_tools()` results, invalidate on MCP restart

### Security Considerations

- **Sandboxing:** Child MCPs run in separate processes (isolation)
- **Capability Filtering:** Gateway can hide dangerous tools
- **Rate Limiting:** Prevent MCP abuse
- **Audit Logging:** Track all tool invocations for compliance

---

## 10. Comparison Table: Composition Approaches

| Approach | Pros | Cons | Startup | Isolation | Use Case |
|----------|------|------|---------|-----------|----------|
| **In-process routers** | Fast, simple, no IPC | Monolithic, no isolation | 0ms | None | Small apps, single-lang |
| **Child processes** | Isolated, polyglot | Slower startup, IPC overhead | 100-500ms | Process-level | Medium apps, mixed langs |
| **HTTP proxy** | Scalable, distributed, language-agnostic | Network latency, complex auth | 50-200ms | Network-level | Enterprise, microservices |
| **Kubernetes gateway** | Auto-scaling, resilient, production-ready | Complex setup, K8s required | Variable | Container-level | Production at scale |
| **Dynamic discovery** | Flexible, on-demand loading | Higher latency, cache complexity | On-demand | Process-level | AI agents exploring |

---

## 11. Next Steps for DX Terminal

### Current State

DX Terminal is an MCP server that spawns **Claude agents** (not MCP servers).
It uses PTY to run `claude` CLI processes.

### How to Add MCP Composition

**Option A: Embed MCP clients in DX Terminal**
- DX Terminal spawns child MCP servers using `TokioChildProcess`
- New tools: `mcp_discover`, `mcp_list_available`, `mcp_spawn`, `mcp_call`
- Agents can discover and use other MCPs through DX Terminal
- All in one binary, simpler deployment

**Option B: Separate MCP Gateway**
- Build new `mcp-gateway` crate
- DX Terminal focuses on agent orchestration
- Gateway focuses on MCP composition
- Clean separation of concerns

**Option C: Hybrid**
- DX Terminal gets basic MCP routing (for agent workflows)
- Separate gateway for advanced features (HTTP, auth, K8s)
- Best of both worlds

### Recommendation: Start with Option A

1. Add `src/mcp_gateway.rs` module to DX Terminal
2. Implement `MCPRegistry` with `TokioChildProcess` spawning
3. Add MCP tools: `mcp_discover`, `mcp_list`, `mcp_spawn`, `mcp_call`
4. Load MCP descriptors from `~/.claude.json` or separate config
5. Agents can now load MCPs on-demand during workflows
6. Later extract to separate crate if scalability demands

**Rationale:** Aligns with DX Terminal mission to orchestrate AI workflows with minimal friction. Agents get MCP composition "for free" without extra setup.

---

## 12. Sources

### Official Documentation
- [Model Context Protocol Specification](https://modelcontextprotocol.io/specification/2025-11-25)
- [MCP Transports](https://modelcontextprotocol.io/legacy/concepts/transports)
- [JSON-RPC Protocol in MCP](https://mcpcat.io/guides/understanding-json-rpc-protocol-mcp/)
- [MCP Best Practices](https://modelcontextprotocol.info/docs/best-practices/)

### Gateway Implementations
- [Microsoft MCP Gateway](https://github.com/microsoft/mcp-gateway)
- [MetaMCP](https://github.com/metatool-ai/metamcp)
- [Agentic Community MCP Gateway](https://github.com/agentic-community/mcp-gateway-registry)

### Composition Patterns
- [FastMCP Server Composition](https://gofastmcp.com/servers/composition)
- [MCP Server Composition Guide](https://medium.com/@sureshddm/mcp-server-composition-build-big-by-thinking-small-adfa826d7440)
- [Dynamic MCP](https://docs.docker.com/ai/mcp-catalog-and-toolkit/dynamic-mcp/)

### Comparison & Analysis
- [10 Best MCP Gateways for Developers in 2026](https://composio.dev/blog/best-mcp-gateway-for-developers)
- [MCP Gateways Guide](https://composio.dev/blog/mcp-gateways-guide)

---

## Appendix: Example Code Locations

- **rmcp source:** `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/rmcp-0.15.0/`
- **DX Terminal MCP implementation:** `/Users/pran/Projects/dx-terminal/src/mcp/mod.rs`
- **DX Terminal uses rmcp v0.15.0** with features: `server`, `transport-io`, `macros`

**Key rmcp files:**
- `src/transport/child_process.rs` - TokioChildProcess for spawning
- `src/transport/io.rs` - stdio() helper (5 lines!)
- `src/handler.rs` - ServerHandler trait
- `src/service.rs` - JSON-RPC 2.0 service layer

---

## Conclusion

MCP composition is mature and well-supported in 2026. Multiple production-ready patterns exist for building modular AI applications where MCPs are lego blocks.

**For DX Terminal:** Adding basic MCP gateway capabilities enables agents to dynamically load tools based on their tasks, making the system more autonomous and capable.

**Next:** Implement `MCPRegistry` + `mcp_discover/spawn/call` tools in DX Terminal.
