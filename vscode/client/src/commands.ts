import { commands, window, workspace, Disposable, Position, Selection, Uri } from "vscode";

export function registerCommands(
    restartServer: () => Promise<void>,
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
        commands.registerCommand("rk.restartServer", restartServer)
    );

    disposables.push(
        commands.registerCommand("rk.showStatusMenu", showStatusMenu)
    );

    return Disposable.from(...disposables);
}
