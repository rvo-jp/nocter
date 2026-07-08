import * as vscode from "vscode";
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
    const config = vscode.workspace.getConfiguration("nocter");
    const command = config.get<string>("server.path", "nocter");
    const args = config.get<string[]>("server.args", ["lsp"]);
    const extraEnv = config.get<Record<string, string>>("server.env", {});

    const serverOptions: ServerOptions = {
        command,
        args,
        options: {
            env: {
                ...process.env,
                ...extraEnv
            }
        }
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [
            {
                scheme: "file",
                language: "nocter"
            }
        ],
        synchronize: {
            configurationSection: "nocter"
        }
    };

    client = new LanguageClient(
        "nocter",
        "Nocter Language Server",
        serverOptions,
        clientOptions
    );

    context.subscriptions.push(client);
    void client.start();
}

export function deactivate(): Thenable<void> | undefined {
    return client?.stop();
}
