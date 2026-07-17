// src/extension.ts
import * as vscode from 'vscode';
import * as child_process from 'child_process';
import * as path from 'path';
import * as fs from 'fs';

let lspClient: vscode.LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext) {
    console.log('🌹 Roze Language extension activated!');

    // Start LSP if enabled
    const config = vscode.workspace.getConfiguration('roze');
    if (config.get('lspEnabled')) {
        startLanguageServer(context);
    }

    // Register build command
    let buildCommand = vscode.commands.registerCommand('roze.build', async () => {
        const terminal = vscode.window.createTerminal('Roze Build');
        terminal.show();
        terminal.sendText('roze-pkg build');
    });

    // Register run command
    let runCommand = vscode.commands.registerCommand('roze.run', async () => {
        const terminal = vscode.window.createTerminal('Roze Run');
        terminal.show();
        terminal.sendText('roze-pkg run');
    });

    // Register new project command
    let newCommand = vscode.commands.registerCommand('roze.new', async () => {
        const name = await vscode.window.showInputBox({
            prompt: 'Enter project name',
            placeHolder: 'my_project'
        });

        if (name) {
            const workspaceFolders = vscode.workspace.workspaceFolders;
            if (workspaceFolders) {
                const folderPath = vscode.Uri.joinPath(workspaceFolders[0].uri, name);
                const terminal = vscode.window.createTerminal('Roze New');
                terminal.show();
                terminal.sendText(`roze-pkg new ${name}`);
            } else {
                vscode.window.showErrorMessage('Please open a workspace folder first');
            }
        }
    });

    // Register restart command
    let restartCommand = vscode.commands.registerCommand('roze.restart', async () => {
        if (lspClient) {
            await lspClient.stop();
            startLanguageServer(context);
            vscode.window.showInformationMessage('🌹 Roze Language Server restarted');
        }
    });

    // Register format on save
    let formatOnSave = vscode.workspace.onDidSaveTextDocument(async (document) => {
        if (document.languageId === 'roze') {
            const config = vscode.workspace.getConfiguration('roze');
            if (config.get('formatOnSave')) {
                // Simple formatting: trim trailing whitespace
                const text = document.getText();
                const formatted = text.split('\n').map(line => line.replace(/\s+$/, '')).join('\n');
                if (text !== formatted) {
                    const edit = new vscode.WorkspaceEdit();
                    const fullRange = new vscode.Range(
                        document.positionAt(0),
                        document.positionAt(text.length)
                    );
                    edit.replace(document.uri, fullRange, formatted);
                    await vscode.workspace.applyEdit(edit);
                    await document.save();
                }
            }
        }
    });

    // Status bar item
    let statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right);
    statusBar.text = '🌹 Roze';
    statusBar.tooltip = 'Roze Language - LSP Active';
    statusBar.command = 'roze.restart';
    statusBar.show();

    context.subscriptions.push(
        buildCommand,
        runCommand,
        newCommand,
        restartCommand,
        formatOnSave,
        statusBar
    );
}

function startLanguageServer(context: vscode.ExtensionContext) {
    // Find the LSP binary
    const lspPath = findLspBinary();
    if (!lspPath) {
        vscode.window.showWarningMessage('Roze LSP binary not found. Install with: cargo build --release -p roze-lsp');
        return;
    }

    // Server options
    const serverOptions: vscode.ServerOptions = {
        run: { command: lspPath, transport: vscode.TransportKind.stdio },
        debug: { command: lspPath, transport: vscode.TransportKind.stdio }
    };

    // Client options
    const clientOptions: vscode.LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'roze' }],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.roze')
        },
        initializationOptions: {},
        diagnosticCollectionName: 'roze'
    };

    // Create and start the client
    lspClient = new vscode.LanguageClient(
        'rozeLsp',
        'Roze Language Server',
        serverOptions,
        clientOptions
    );

    lspClient.start();
    vscode.window.showInformationMessage('🌹 Roze Language Server started');
}

function findLspBinary(): string | undefined {
    // Check in the project root
    const possiblePaths = [
        path.join(__dirname, '../../../target/release/roze-lsp'),
        path.join(__dirname, '../target/release/roze-lsp'),
        'roze-lsp'
    ];

    for (const p of possiblePaths) {
        try {
            if (fs.existsSync(p) || child_process.spawnSync('which', [p]).status === 0) {
                return p;
            }
        } catch {
            // Continue
        }
    }

    return undefined;
}

export function deactivate() {
    if (lspClient) {
        lspClient.stop();
    }
    console.log('🌹 Roze Language extension deactivated!');
}