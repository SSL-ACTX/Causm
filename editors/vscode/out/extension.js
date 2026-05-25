"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.deactivate = exports.activate = void 0;
const fs = require("fs");
const path = require("path");
const vscode_1 = require("vscode");
const child_process_1 = require("child_process");
const node_1 = require("vscode-languageclient/node");
let client;
function isExecutable(filePath) {
    try {
        const stat = fs.statSync(filePath);
        return stat.isFile();
    }
    catch {
        return false;
    }
}
function resolveViaWhich(command) {
    const resolver = process.platform === 'win32' ? 'where' : 'which';
    try {
        const stdout = (0, child_process_1.execSync)(`${resolver} ${command}`, {
            stdio: ['ignore', 'pipe', 'ignore'],
            encoding: 'utf8',
            timeout: 5000
        });
        for (const line of stdout.split(/\r?\n/)) {
            const candidate = line.trim();
            if (candidate && isExecutable(candidate)) {
                return candidate;
            }
        }
    }
    catch {
        // ignore missing command or error
    }
    return undefined;
}
function resolveCausmLspPath(context) {
    const config = vscode_1.workspace.getConfiguration('causm');
    const configPath = config.get('lsp.path');
    const executableName = process.platform === 'win32' ? 'causm-lsp.exe' : 'causm-lsp';
    if (configPath) {
        if (path.isAbsolute(configPath) && isExecutable(configPath)) {
            return configPath;
        }
        return configPath;
    }
    // Try PATH lookup first.
    const fromPath = resolveViaWhich(executableName);
    if (fromPath) {
        return fromPath;
    }
    const candidates = [];
    if (vscode_1.workspace.workspaceFolders) {
        for (const folder of vscode_1.workspace.workspaceFolders) {
            candidates.push(path.join(folder.uri.fsPath, 'target', 'debug', executableName));
            candidates.push(path.join(folder.uri.fsPath, 'target', 'release', executableName));
            candidates.push(path.join(folder.uri.fsPath, 'target', executableName));
        }
    }
    const extensionPath = context.extensionUri.fsPath;
    candidates.push(path.join(extensionPath, '..', '..', 'target', 'debug', executableName));
    candidates.push(path.join(extensionPath, '..', '..', 'target', 'release', executableName));
    candidates.push(path.join(process.cwd(), 'target', 'debug', executableName));
    candidates.push(path.join(process.cwd(), 'target', 'release', executableName));
    for (const candidate of candidates) {
        if (candidate && isExecutable(candidate)) {
            return candidate;
        }
    }
    // No valid binary found.
    return undefined;
}
function activate(context) {
    const lspPath = resolveCausmLspPath(context);
    if (!lspPath) {
        const msg = 'Could not resolve causm-lsp path (looked for setting, workspace targets, extension path, and PATH).\n' +
            'Run `cargo build --bin causm-lsp` and set `causm.lsp.path` explicitly if needed.';
        vscode_1.window.showErrorMessage(msg);
        console.error(msg);
        return;
    }
    vscode_1.window.showInformationMessage(`Using causm-lsp binary: ${lspPath}`);
    const serverOptions = {
        run: { command: lspPath },
        debug: { command: lspPath }
    };
    const clientOptions = {
        documentSelector: [{ scheme: 'file', language: 'causm' }],
        synchronize: {
            fileEvents: vscode_1.workspace.createFileSystemWatcher('**/*.csm')
        }
    };
    client = new node_1.LanguageClient('causmLanguageServer', 'Causm Language Server', serverOptions, clientOptions);
    client.start();
}
exports.activate = activate;
function deactivate() {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
exports.deactivate = deactivate;
//# sourceMappingURL=extension.js.map