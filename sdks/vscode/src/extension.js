// Pick AI - VS Code Extension
// Connects to the local Pick server and provides AI assistance

const vscode = require('vscode');

let serverUrl = 'http://127.0.0.1:8080';

/**
 * @param {vscode.ExtensionContext} context
 */
function activate(context) {
    console.log('Pick AI extension activating...');

    // Load server URL from settings
    const config = vscode.workspace.getConfiguration('pick');
    serverUrl = config.get('serverUrl', 'http://127.0.0.1:8080');

    // Register commands
    const startSession = vscode.commands.registerCommand('pick.startSession', async () => {
        const panel = vscode.window.createWebviewPanel(
            'pickChat',
            'Pick AI - Chat',
            vscode.ViewColumn.Beside,
            { enableScripts: true }
        );

        panel.webview.html = getWebviewContent();
    });

    const askPrompt = vscode.commands.registerCommand('pick.askPrompt', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
            vscode.window.showInformationMessage('No active editor');
            return;
        }

        const selection = editor.selection;
        const selectedText = editor.document.getText(selection);

        if (!selectedText) {
            vscode.window.showInformationMessage('No code selected');
            return;
        }

        const prompt = await vscode.window.showInputBox({
            prompt: 'What would you like to ask about the selected code?',
            placeHolder: 'e.g., Explain this code'
        });

        if (!prompt) return;

        // Send to Pick server
        try {
            const res = await fetch(`${serverUrl}/ask`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    session_id: await createSession(),
                    prompt: `Selected code:\n\`\`\`\n${selectedText}\n\`\`\`\n\n${prompt}`
                })
            });
            const result = await res.text();
            vscode.window.showInformationMessage(`Pick: ${result}`);
        } catch (err) {
            vscode.window.showErrorMessage(`Pick error: ${err.message}`);
        }
    });

    context.subscriptions.push(startSession, askPrompt);
    console.log('Pick AI extension activated');
}

function deactivate() {}

async function createSession() {
    const res = await fetch(`${serverUrl}/sessions`, { method: 'POST' });
    const data = await res.json();
    return data.session_id;
}

function getWebviewContent() {
    return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Pick Chat</title>
    <style>
        body { font-family: var(--vscode-font-family); padding: 10px; }
        #messages { margin-bottom: 10px; }
        .message { padding: 8px; margin: 4px 0; border-radius: 4px; }
        .user { background: var(--vscode-input-background); }
        .assistant { background: var(--vscode-editor-background); }
        #input { width: 100%; padding: 8px; box-sizing: border-box; }
    </style>
</head>
<body>
    <div id="messages"></div>
    <input id="input" type="text" placeholder="Ask Pick..." />
    <script>
        const input = document.getElementById('input');
        const messages = document.getElementById('messages');
        input.addEventListener('keypress', async (e) => {
            if (e.key === 'Enter') {
                const msg = document.createElement('div');
                msg.className = 'message user';
                msg.textContent = input.value;
                messages.appendChild(msg);
                input.value = '';
            }
        });
    </script>
</body>
</html>`;
}

module.exports = { activate, deactivate };
