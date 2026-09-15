import {
  ExtensionContext,
  ProcessExecution,
  StatusBarAlignment,
  StatusBarItem,
  Task,
  TaskGroup,
  TaskProvider,
  TaskRevealKind,
  tasks,
  ThemeColor,
  Uri,
  WebviewPanel,
  ViewColumn,
  window,
  workspace,
  WorkspaceFolder,
} from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  Executable,
} from "vscode-languageclient/node";
import { rkCommand, rememberExtension } from "./rk";
import { registerCommands } from "./commands";
import { execFileSync } from "child_process";

let client: LanguageClient;
let statusBarItem: StatusBarItem;
let currentStatus: ServerStatus = { status: "ok", message: "" };
let statusMessages: StatusMessage[] = [];
let statusPanel: WebviewPanel | undefined;
let outputChannel: import("vscode").OutputChannel;

interface ServerStatus {
  status: "ok" | "warning" | "error";
  message: string;
}

interface StatusMessage {
  status: "ok" | "warning" | "error";
  message: string;
  timestamp: Date;
}

export async function activate(context: ExtensionContext) {
  rememberExtension(context);
  // A build task, offered for `Run Task`, registered before the language
  // server so it exists even when the server fails to start.
  context.subscriptions.push(
    tasks.registerTaskProvider(
      RkBuildTaskProvider.TYPE,
      new RkBuildTaskProvider(),
    ),
  );

  const serverBinary =
    process.platform === "win32"
      ? "vscode-lsp-server.exe"
      : "vscode-lsp-server";

  const serverModule = Uri.joinPath(
    context.extensionUri,
    "server",
    "bin",
    serverBinary,
  );

  const env = {
    ...process.env,
    RUST_BACKTRACE: "1",
    ...stdlibEnv(),
  };

  outputChannel = window.createOutputChannel("IEC LSP Server", "log");
  const run: Executable = {
    command: serverModule.fsPath,
    options: { env },
  };
  const serverOptions: ServerOptions = {
    run,
    debug: run,
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "st" }],
    // File watching is handled by server-side dynamic registration
    outputChannel: outputChannel,
  };

  // Create status bar item
  statusBarItem = window.createStatusBarItem(StatusBarAlignment.Left, 0);
  statusBarItem.command = "rk.showStatusMenu";
  updateStatusBar({ status: "ok", message: "" });
  statusBarItem.show();
  context.subscriptions.push(statusBarItem);

  client = new LanguageClient(
    "lspClient",
    "LSP Client",
    serverOptions,
    clientOptions,
  );

  try {
    await client.start();
  } catch (error) {
    client.error(`Start failed`, error, "force");
    updateStatusBar({ status: "error", message: "Server failed to start" });
  }

  // Listen for server status notifications
  client.onNotification("rk/serverStatus", (params: ServerStatus) => {
    updateStatusBar(params);
  });

  // Register commands
  const commandsDisposable = registerCommands(restartServer, stopServer, showStatusMenu);
  context.subscriptions.push(commandsDisposable);
}

/// The standard library the INSTALLED toolchain resolved, as environment for
/// the server.
///
/// The extension ships its own server binary, so client and server versions
/// always match, but it must not ship its own standard library: a second copy
/// is a second answer, and the editor would disagree with `rk check` about the
/// same source. So it asks the toolchain rather than guessing, the way
/// rust-analyzer asks rustc for its sysroot.
///
/// Nothing is set when the CLI cannot be reached: the server then finds a
/// library beside itself, or reports that it found none. A missing `rk` is
/// already visible elsewhere.
function stdlibEnv(): Record<string, string> {
  try {
    // `rk env <key>` prints the value alone, the way `go env GOROOT` does.
    // Nothing to parse, so a path with a space or a parenthesis in it
    // survives.
    const resolved = execFileSync(rkCommand(), ["env", "stdlib"], {
      encoding: "utf8",
      timeout: 5000,
    }).trim();
    if (resolved) {
      return { RK_STDLIB_PATH: resolved };
    }
  } catch {
    // Not installed, not on PATH, or too slow to answer.
  }
  return {};
}

/// Contributes an "rk: build (debug)" task: compiles the workspace with the
/// debug profile, the way `rk compile` does.
class RkBuildTaskProvider implements TaskProvider {
  static readonly TYPE = "rk";
  static readonly NAME = "build (debug)";

  provideTasks(): Task[] {
    return (workspace.workspaceFolders ?? []).map((f) => buildDebugTask(f));
  }

  resolveTask(task: Task): Task | undefined {
    const folder =
      typeof task.scope === "object"
        ? (task.scope as WorkspaceFolder)
        : workspace.workspaceFolders?.[0];
    if (!folder) {
      return undefined;
    }
    return buildDebugTask(folder, task.definition);
  }
}

function buildDebugTask(
  folder: WorkspaceFolder,
  definition?: { type: string; workspace?: string },
): Task {
  const ws = definition?.workspace ?? folder.uri.fsPath;
  const task = new Task(
    definition ?? { type: RkBuildTaskProvider.TYPE, workspace: ws },
    folder,
    RkBuildTaskProvider.NAME,
    RkBuildTaskProvider.TYPE,
    // ProcessExecution (like rust-analyzer) — spawn `rk` directly, no shell.
    // `--workspace` is a FLAG, not a positional. `--debug` is now the default
    // profile and only a hidden alias, so the artifact path is unchanged.
    new ProcessExecution(rkCommand(), ["compile", "--workspace", ws]),
    [],
  );
  task.group = TaskGroup.Build;
  // Reveal the terminal so the user sees compilation; clear stale output.
  task.presentationOptions = { reveal: TaskRevealKind.Always, clear: true };
  return task;
}

function updateStatusBar(status: ServerStatus) {
  currentStatus = status;

  if (status.message) {
    statusMessages.push({
      ...status,
      timestamp: new Date(),
    });

    // Keep last 50 messages
    if (statusMessages.length > 50) {
      statusMessages = statusMessages.slice(-50);
    }
  }

  switch (status.status) {
    case "ok":
      statusBarItem.text = "$(check) IEC 61131-3";
      statusBarItem.tooltip = "IEC 61131-3 — Server running";
      statusBarItem.backgroundColor = undefined;
      break;
    case "warning":
      statusBarItem.text = "$(warning) IEC 61131-3";
      statusBarItem.tooltip = `IEC 61131-3 — ${status.message}`;
      statusBarItem.backgroundColor = new ThemeColor(
        "statusBarItem.warningBackground",
      );
      break;
    case "error":
      statusBarItem.text = "$(error) IEC 61131-3";
      statusBarItem.tooltip = `IEC 61131-3 — ${status.message}`;
      statusBarItem.backgroundColor = new ThemeColor(
        "statusBarItem.errorBackground",
      );
      break;
  }

  // Update panel if open
  if (statusPanel) {
    statusPanel.webview.html = getStatusHtml();
  }
}

async function showStatusMenu() {
  if (statusPanel) {
    statusPanel.reveal();
    return;
  }

  statusPanel = window.createWebviewPanel(
    "iecStatus",
    "IEC 61131-3 — Server Status",
    { viewColumn: ViewColumn.Active, preserveFocus: true },
    { enableScripts: true },
  );

  statusPanel.webview.html = getStatusHtml();

  statusPanel.webview.onDidReceiveMessage(async (msg) => {
    if (msg.command === "restart") {
      await restartServer();
    } else if (msg.command === "stop") {
      await stopServer();
    } else if (msg.command === "showLogs") {
      outputChannel.show();
    }
  });

  statusPanel.onDidDispose(() => {
    statusPanel = undefined;
  });
}

function getStatusHtml(): string {
  const statusIcon =
    currentStatus.status === "ok"
      ? "✓"
      : currentStatus.status === "warning"
        ? "⚠"
        : "✗";

  const statusColor =
    currentStatus.status === "ok"
      ? "var(--vscode-testing-iconPassed)"
      : currentStatus.status === "warning"
        ? "var(--vscode-editorWarning-foreground)"
        : "var(--vscode-editorError-foreground)";

  const messagesHtml = statusMessages
    .slice()
    .reverse()
    .map((msg) => {
      const icon = msg.status === "ok" ? "✓" : msg.status === "warning" ? "⚠" : "✗";
      const color =
        msg.status === "ok"
          ? "var(--vscode-testing-iconPassed)"
          : msg.status === "warning"
            ? "var(--vscode-editorWarning-foreground)"
            : "var(--vscode-editorError-foreground)";
      const time = msg.timestamp.toLocaleTimeString();
      return `<div class="message">
        <span class="icon" style="color:${color}">${icon}</span>
        <span class="time">${time}</span>
        <span class="text">${escapeHtml(msg.message)}</span>
      </div>`;
    })
    .join("");

  return `<!DOCTYPE html>
<html>
<head>
  <style>
    body {
      font-family: var(--vscode-font-family);
      font-size: var(--vscode-font-size);
      color: var(--vscode-foreground);
      background: var(--vscode-editor-background);
      padding: 16px;
      margin: 0;
    }
    .header {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 16px;
      padding-bottom: 12px;
      border-bottom: 1px solid var(--vscode-widget-border);
    }
    .header .icon {
      font-size: 24px;
    }
    .header .title {
      font-size: 16px;
      font-weight: 600;
    }
    .header .status-text {
      color: var(--vscode-descriptionForeground);
    }
    .actions {
      margin-bottom: 16px;
    }
    button {
      background: var(--vscode-button-background);
      color: var(--vscode-button-foreground);
      border: none;
      padding: 6px 14px;
      cursor: pointer;
      font-size: var(--vscode-font-size);
      border-radius: 2px;
    }
    button:hover {
      background: var(--vscode-button-hoverBackground);
    }
    .messages-header {
      font-weight: 600;
      margin-bottom: 8px;
      color: var(--vscode-descriptionForeground);
    }
    .message {
      display: flex;
      align-items: baseline;
      gap: 8px;
      padding: 4px 0;
      font-family: var(--vscode-editor-font-family);
      font-size: var(--vscode-editor-font-size);
    }
    .message .icon {
      flex-shrink: 0;
      width: 16px;
      text-align: center;
    }
    .message .time {
      flex-shrink: 0;
      color: var(--vscode-descriptionForeground);
      font-size: 0.9em;
    }
    .message .text {
      word-break: break-word;
    }
    .empty {
      color: var(--vscode-descriptionForeground);
      font-style: italic;
    }
  </style>
</head>
<body>
  <div class="header">
    <span class="icon" style="color:${statusColor}">${statusIcon}</span>
    <div>
      <div class="title">IEC 61131-3 Language Server</div>
      <div class="status-text">${currentStatus.status === "ok" ? "Server running" : escapeHtml(currentStatus.message)}</div>
    </div>
  </div>
  <div class="actions">
    <button onclick="restart()">Restart Server</button>
    <button onclick="stop()">Stop Server</button>
    <button onclick="showLogs()">Show Logs</button>
  </div>
  <div class="messages-header">Messages</div>
  ${messagesHtml || '<div class="empty">No messages</div>'}
  <script>
    const vscode = acquireVsCodeApi();
    function restart() {
      vscode.postMessage({ command: 'restart' });
    }
    function stop() {
      vscode.postMessage({ command: 'stop' });
    }
    function showLogs() {
      vscode.postMessage({ command: 'showLogs' });
    }
  </script>
</body>
</html>`;
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

async function stopServer() {
  statusBarItem.text = "$(sync~spin) IEC 61131-3";
  statusBarItem.tooltip = "IEC 61131-3 — Stopping server...";

  try {
    if (client) {
      await client.stop();
      statusBarItem.text = "$(circle-slash) IEC 61131-3";
      statusBarItem.tooltip = "IEC 61131-3 — Server stopped";
      statusBarItem.backgroundColor = undefined;
    }
  } catch (error) {
    updateStatusBar({ status: "error", message: "Server failed to stop" });
    window.showErrorMessage(`Failed to stop IEC server: ${error}`);
  }
}

async function restartServer() {
  updateStatusBar({ status: "ok", message: "" });
  statusBarItem.text = "$(sync~spin) IEC 61131-3";
  statusBarItem.tooltip = "IEC 61131-3 — Restarting server...";

  try {
    if (client) {
      await client.restart();
    }
  } catch (error) {
    updateStatusBar({ status: "error", message: "Server failed to restart" });
    window.showErrorMessage(`Failed to restart IEC server: ${error}`);
  }
}

export function deactivate(): Thenable<void> | undefined {
  return client.stop();
}
