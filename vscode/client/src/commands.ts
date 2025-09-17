import { commands, window, workspace, Range, Position, Selection, Uri } from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";

export function registerCommands(client: LanguageClient) {
    const showImplementationsCommand = commands.registerCommand("rk.showImplementations", async (uri?: string, range?: any) => {
        try {
            // If no parameters provided, use current editor and cursor position
            if (!uri || !range) {
                return await commands.executeCommand("editor.action.goToImplementation");
            }
            
            // Open the document and position cursor at the specified range
            const document = await workspace.openTextDocument(Uri.parse(uri));
            const editor = await window.showTextDocument(document);
            const position = new Position(range.line, range.character);
            editor.selection = new Selection(position, position);
            
            return await commands.executeCommand("editor.action.goToImplementation");
        } catch (error) {
            window.showErrorMessage(`Failed to show implementations: ${error}`);
            return null;
        }
    });

    return showImplementationsCommand;
}
