const NOCTER_LANGUAGES = new Set(["nct", "nocter"]);
const SHELL_LANGUAGES = new Set(["sh", "shell", "bash", "zsh"]);

function highlightCode(source, language = "") {
    const normalized = String(language).toLowerCase();

    if (NOCTER_LANGUAGES.has(normalized)) {
        return highlightNocterCode(source);
    }

    if (SHELL_LANGUAGES.has(normalized)) {
        return highlightShellCode(source);
    }

    return escapeHtml(source);
}

function highlightNocterCode(source) {
    const pattern = /\/\/.*|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|&\+|\.{3}|\b[A-Za-z_][A-Za-z0-9_]*\b|\b\d+\b|[{}()[\]:=.+*/<>,&?!]/g;
    const keywords = new Set(["use", "struct", "interface", "impl", "enum", "literal", "construct", "func", "method", "pub", "primitive", "let", "var", "move", "as", "otherwise", "match", "return", "if", "else", "for", "in"]);
    const types = new Set(["u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "isize", "str", "bool", "void", "never", "Self"]);
    let output = "";
    let lastIndex = 0;

    for (const match of source.matchAll(pattern)) {
        const token = match[0];
        const index = match.index;
        output += escapeHtml(source.slice(lastIndex, index));
        output += highlightNocterToken(token, source.slice(index + token.length), keywords, types);
        lastIndex = index + token.length;
    }

    return output + escapeHtml(source.slice(lastIndex));
}

function highlightNocterToken(value, rest, keywords, types) {
    if (value.startsWith("//")) return syntaxToken("comment", value);
    if (value.startsWith('"') || value.startsWith("'")) return syntaxToken("string", value);
    if (/^\d+$/.test(value)) return syntaxToken("number", value);
    if (keywords.has(value)) return syntaxToken("keyword", value);
    if (types.has(value) || /^[A-Z][A-Za-z0-9_]*$/.test(value)) return syntaxToken("type", value);
    if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(value) && rest.trimStart().startsWith("(")) return syntaxToken("function", value);
    if (value === "&+" || value === "..." || /^[{}()[\]:=.+*/<>,&?!]$/.test(value)) return syntaxToken("operator", value);
    return escapeHtml(value);
}

function highlightShellCode(source) {
    return String(source)
        .split(/(\n)/)
        .map(part => part === "\n" ? part : highlightShellLine(part))
        .join("");
}

function highlightShellLine(line) {
    let output = "";
    let index = 0;
    let commandPosition = true;
    const pattern = /#[^\n]*|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|\$\{?[A-Za-z_][A-Za-z0-9_]*\}?|--?[A-Za-z0-9][A-Za-z0-9_-]*|\b[A-Za-z_][A-Za-z0-9_-]*\b|\b\d+\b|[|&;()<>]/g;
    const keywords = new Set(["if", "then", "else", "elif", "fi", "for", "in", "do", "done", "case", "esac", "while", "until", "function"]);
    const builtins = new Set(["cd", "echo", "printf", "test", "export", "local", "readonly", "unset", "shift", "return", "exit", "source", "alias", "command"]);

    for (const match of line.matchAll(pattern)) {
        const token = match[0];
        const tokenIndex = match.index;
        const gap = line.slice(index, tokenIndex);
        output += escapeHtml(gap);

        if (/\S/.test(gap) && !/[|&;(]\s*$/.test(gap)) {
            commandPosition = false;
        }

        output += highlightShellToken(token, keywords, builtins, commandPosition);

        if (/^[|&;(]$/.test(token)) {
            commandPosition = true;
        } else if (!/^#/.test(token)) {
            commandPosition = false;
        }

        index = tokenIndex + token.length;

        if (token.startsWith("#")) {
            break;
        }
    }

    return output + escapeHtml(line.slice(index));
}

function highlightShellToken(value, keywords, builtins, commandPosition) {
    if (value.startsWith("#")) return syntaxToken("comment", value);
    if (value.startsWith('"') || value.startsWith("'")) return syntaxToken("string", value);
    if (value.startsWith("$")) return syntaxToken("variable", value);
    if (/^--?/.test(value)) return syntaxToken("option", value);
    if (/^\d+$/.test(value)) return syntaxToken("number", value);
    if (keywords.has(value)) return syntaxToken("keyword", value);
    if (builtins.has(value) || commandPosition) return syntaxToken("function", value);
    if (/^[|&;()<>]$/.test(value)) return syntaxToken("operator", value);
    return escapeHtml(value);
}

function syntaxToken(type, value) {
    return `<span class="token-${type}">${escapeHtml(value)}</span>`;
}

function escapeHtml(text) {
    return String(text).replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

module.exports = {
    highlightCode,
    highlightNocterCode,
    highlightShellCode
};
