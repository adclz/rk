import { commands, window, workspace, Disposable, Position, Selection, Uri, Task, TaskScope, TaskRevealKind, TaskPanelKind, ShellExecution, tasks } from "vscode";

export function registerCommands(
    restartServer: () => Promise<void>,
    stopServer: () => Promise<void>,
    showStatusMenu: () => Promise<void>,
) {
    const disposables: Disposable[] = [];

    disposables.push(
        commands.registerCommand("rk.showImplementations", async (uri?: string, range?: any) => {
            try {
                if (!uri || !range) {
                    return await commands.executeCommand("editor.action.goToImplementation");
                }
                const document = await workspace.openTextDocument(Uri.parse(uri));
                const editor = await window.showTextDocument(document);
                const position = new Position(range.line, range.character);
                editor.selection = new Selection(position, position);
                return await commands.executeCommand("editor.action.goToImplementation");
            } catch (error) {
                window.showErrorMessage(`Failed to show implementations: ${error}`);
                return null;
            }
        })
    );

    disposables.push(
        commands.registerCommand("rk.runTest", async (uri?: string, testName?: string) => {
            if (!uri || !testName) {
                window.showWarningMessage("No test to run");
                return;
            }

            const workspaceFolder = workspace.workspaceFolders?.[0]?.uri.fsPath;
            if (!workspaceFolder) {
                window.showErrorMessage("No workspace folder found");
                return;
            }

            const bareName = testName.includes(".") ? testName.split(".").pop()! : testName;

            const task = new Task(
                { type: "rk-test", testName },
                TaskScope.Workspace,
                bareName,
                "rk",
                new ShellExecution(`rk test "${testName}"`, { cwd: workspaceFolder }),
            );
            task.presentationOptions = { reveal: TaskRevealKind.Always, panel: TaskPanelKind.Dedicated };
            await tasks.executeTask(task);
        })
    );

    disposables.push(
        commands.registerCommand("rk.restartServer", restartServer)
    );

    disposables.push(
        commands.registerCommand("rk.stopServer", stopServer)
    );

    disposables.push(
        commands.registerCommand("rk.showStatusMenu", showStatusMenu)
    );

    return Disposable.from(...disposables);
}
