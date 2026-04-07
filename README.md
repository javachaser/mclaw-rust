# MClaw

> OpenClaw 的本地桌面管理器（Rust 版）

MClaw 是一个面向 Windows 的桌面应用，用来管理本机 OpenClaw Gateway 的启动、停止、状态检测、配置查看和日志查看。

当前实现基于 Rust 重写，采用单文件桌面应用思路：后端使用 `axum` 提供本地 HTTP 服务，前端界面内嵌到 WebView 中，通过 `winit + wry` 提供原生桌面窗口。

## 功能特性

- 管理本机 OpenClaw Gateway 的启动与停止
- 检测 OpenClaw 安装状态与运行健康状态
- 查看当前服务端口、PID 与访问地址
- 读取并展示 OpenClaw 配置、模型配置、认证信息（界面侧会做脱敏展示）
- 查看最近日志输出，便于排查启动与运行问题
- 支持打开独立的 OpenClaw 控制台窗口
- 内置单实例唤醒逻辑，重复启动时会唤醒已有窗口而不是打开多个实例
- 在 Windows 上对子进程启用隐藏执行，避免弹出额外控制台窗口

## 技术栈

- Rust 2021
- axum
- tokio
- reqwest
- winit
- wry / WebView2
- Inno Setup

## 运行架构

MClaw 由两部分组成：

1. **本地管理服务**
   - 默认监听：`127.0.0.1:19000`
   - 提供状态、启动、停止、日志、配置等本地接口
2. **桌面窗口**
   - 通过 WebView 加载本地管理界面
   - 支持从主窗口打开独立控制台窗口

被管理的 OpenClaw Gateway 默认端口为：`18789`

## 系统要求

- Windows 10 / 11 x64
- 已安装 Microsoft Edge WebView2 Runtime
- 已安装并可运行的 OpenClaw 环境
- 若从源码构建安装包，需要：
  - Rust 工具链
  - `x86_64-pc-windows-gnu` 目标
  - Inno Setup 6
  - MinGW（用于 GNU 目标构建）

## 安装方式

### 方式一：直接下载安装包

前往仓库的 Release 页面下载最新安装包：

- `MClaw-Setup-1.0.2-x64.exe`

安装程序默认会把应用安装到：

- `{localappdata}\Programs\MClaw`

安装过程中可以改成你自己选择的目录。

如果系统尚未安装 WebView2 Runtime，安装程序会自动尝试安装。


### 方式二：从源码运行

```powershell
cargo run
```

## 从源码构建

### 调试构建

```powershell
cargo build
```

### 发布构建（GNU 目标）

```powershell
cargo +stable-x86_64-pc-windows-gnu build --release --target x86_64-pc-windows-gnu
```

### 构建安装包

```powershell
powershell -ExecutionPolicy Bypass -File .\installer\build-installer.ps1
```

安装包输出目录：

- `dist/`

## 使用说明

启动 MClaw 后，主界面会通过本地管理服务展示当前 OpenClaw 的状态。典型使用流程如下：

1. 打开应用，检查 OpenClaw 是否已正确安装
2. 点击启动按钮拉起 OpenClaw Gateway
3. 在界面中查看健康状态、PID、访问地址和端口
4. 按需查看模型配置、运行配置与日志
5. 如需进入控制台，可从桌面窗口中直接打开

## 目录结构

```text
mclaw-rust/
├─ assets/
│  ├─ index.html              # 内嵌前端界面
│  ├─ MClaw_Logo.png          # 应用 logo 原图
│  └─ MClaw.ico               # Windows 安装器与快捷方式图标
├─ installer/
│  ├─ build-installer.ps1     # 安装包构建脚本
│  ├─ MClaw.iss               # Inno Setup 脚本
│  └─ payload/                # 安装包附带资源
├─ src/
│  └─ main.rs                 # 桌面应用与本地服务主逻辑
├─ build.rs                   # 构建期处理 WebView2Loader.dll
├─ Cargo.toml                 # 项目元数据与依赖
└─ README.md
```

## 本地接口概览

MClaw 内置的本地服务主要提供以下接口：

- `GET /api/status`
- `GET /api/detect`
- `POST /api/start`
- `POST /api/stop`
- `GET /api/models`
- `GET /api/config`
- `GET /api/logs`
- `POST /api/activate`

这些接口主要用于本机桌面界面调用，不建议直接暴露到公网。

## 当前实现说明

当前版本更偏向"面向本机既有 OpenClaw 环境的管理器"，也就是说：

- 默认依赖本机 OpenClaw 的既定安装与配置位置
- 主要面向 Windows 本地使用场景
- 适合作为 OpenClaw 的轻量桌面外壳和运维面板

如果后续继续演进，可以进一步把路径、端口和运行参数做成可配置项。

## 版本

当前版本：`1.0.2`

## 许可证

当前仓库尚未补充明确许可证文件；如果你准备公开长期维护，建议尽快补上 `LICENSE`。
