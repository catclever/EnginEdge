# EnginEdge: 产品需求与技术规范 (中文文档)

> **定位**：面向 Descartes 与 Dalembert 生态的高性能、单静态二进制端侧执行引擎与 Model Context Protocol (MCP) 服务端。
> **架构哲学**：*"Soul in Cloud, Body at Edge"（云端构筑灵魂，边缘驱动肉身）* —— 云端集中式模型推理与开源情报调研，端侧零依赖原生语法树操作与安全沙箱执行。

---

## 1. 项目愿景与痛点分析

在现代 AI 辅助编码流程中（如使用 Cursor、Claude Desktop、Antigravity 或 Zed），传统代码 Agent 面临两大严峻挑战：
1. **行号读写的脆弱性**：依靠行号（如 `start_line: 10, end_line: 45`）极易受到代码插入或删除的**行号漂移（Line-Drift）**影响，导致大模型频繁发生越界删除或上下文错位；
2. **本地环境的笨重与资源消耗**：在开发者本机运行庞大的 Python 或 Node.js Agent 运行时，不仅消耗宝贵的内存与电量，还容易引入繁杂的环境依赖冲突。

**EnginEdge** 通过纯 Rust 打造的轻量级单二进制客户端彻底解决这些问题：
- **原生标准 MCP 服务**：通过 `stdio` 协议运行，直接即插即用于 Cursor、Claude Desktop 及 Antigravity；
- **基于 Tree-sitter 的符号级语法树引擎**：在 AST 抽象语法树层级进行大纲提取、符号精读与符号精准替换，彻底告别脆弱的行号；
- **云端大脑长连接桥梁**：通过 WebSocket 与云端的 **Descartes Produit** 和 **Dalembert** 实时联动，按需获取全网开源蓝图、官方技术文档与测试用例；
- **本地防窃密安全牢笼**：强制限制工作区路径，自动阻断并脱敏敏感凭据（`.env`、SSH 私钥、Token 等），严防凭证泄露。

---

## 2. 核心能力模块矩阵

### 模块 A：MCP 服务端与宿主接入层
- **协议契约**：遵循 Model Context Protocol (MCP) 最新规范，基于标准输入输出（`stdio`）进行 JSON-RPC 2.0 通信。
- **宿主生态**：全面支持 Cursor、Claude Desktop、Antigravity、Zed 及 Windsurf。
- **输出洁净度保证**：内部调试日志严格输出至 `stderr`，确保 `stdout` 仅包含纯净的 JSON-RPC 帧。

### 模块 B：跨语言通用 Tree-sitter 符号引擎
- **支持语言（一期）**：Rust、Go、TypeScript/JavaScript、Python、Ruby。
- **核心工具**：
  - `get_outline(file_path)`：毫秒级提取文件大纲（类、结构体、函数签名、接口），无需将整篇文件灌入上下文；
  - `read_symbol(file_path, symbol_path)`：精准提取指定符号（如 `Server::handle_message`）的完整实现代码；
  - `replace_symbol(file_path, symbol_path, new_code)`：在 AST 节点位置做原子替换，自动进行语法校验与缩进格式化。

### 模块 C：云端情报连接中枢 (WebSocket Bridge)
- 连通云端的 [Descartes Produit](https://github.com/catclever/descartes-produit) 与 [Dalembert](https://github.com/catclever/Dalembert)；
- 向本地 IDE 暴露云端情报检索能力：
  - `survey_architecture`：检索开源高星项目蓝图与架构设计；
  - `search_code_examples`：搜索真实世界开源代码调用样例；
  - `fetch_doc`：提取纯净官方技术文档；
  - `inspect_spec`：抓取测试用例作为第一手可执行规约。

### 模块 D：本地安全沙箱防线 (Anti-Leak Jail)
- **目录牢笼（Chroot 语义）**：所有文件读写严格限定在当前工作目录（`$PWD`），严禁越狱；
- **敏感文件黑名单**：强行拦截对 `.env*`、`id_rsa*`、`*.pem`、`*.key`、`~/.aws`、`~/.ssh` 的访问；
- **密钥正则脱敏**：回传前自动过滤并替换常见 Token 为 `[REDACTED_SECRET]`。

---

## 3. 技术选型

- **核心语言**：Rust (Edition 2021+)
- **异步底座**：`tokio` (features = ["full"])
- **JSON / RPC**：`serde`, `serde_json`
- **语法树引擎**：`tree-sitter` 及各语言 grammar 库
- **网络通信**：`tokio-tungstenite` (异步 WebSocket 客户端)
- **参数解析**：`clap` (derive 模式)
