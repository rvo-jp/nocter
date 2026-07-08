import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
    const config = vscode.workspace.getConfiguration("nocter");
    const configuredCommand = config.get<string>("server.path", "").trim();
    const command = configuredCommand || defaultServerPath(context.extensionPath);
    const args = config.get<string[]>("server.args", ["lsp"]);
    const extraEnv = config.get<Record<string, string>>("server.env", {});
    const defaultEnv = defaultServerEnv(context.extensionPath);

    const serverOptions: ServerOptions = {
        command,
        args,
        options: {
            env: {
                ...process.env,
                ...defaultEnv,
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

function defaultServerPath(extensionPath: string): string {
    const candidates = [
        path.resolve(extensionPath, "..", "..", "compiler", "target", "debug", binaryName()),
        path.resolve(extensionPath, "..", "..", ".nocter", binaryName())
    ];

    return candidates.find(isExecutableFile) ?? "nocter";
}

function defaultServerEnv(extensionPath: string): Record<string, string> {
    const repoNocterHome = path.resolve(extensionPath, "..", "..", ".nocter");
    if (isDirectory(repoNocterHome) && process.env.NOCTER_HOME === undefined) {
        return {
            NOCTER_HOME: repoNocterHome
        };
    }

    return {};
}

function binaryName(): string {
    return process.platform === "win32" ? "nocter.exe" : "nocter";
}

function isExecutableFile(file: string): boolean {
    try {
        return fs.statSync(file).isFile();
    } catch {
        return false;
    }
}

function isDirectory(directory: string): boolean {
    try {
        return fs.statSync(directory).isDirectory();
    } catch {
        return false;
    }
}
