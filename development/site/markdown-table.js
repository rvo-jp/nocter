// Owns Markdown table cell boundaries. Rendering consumes these cells and never reinterprets
// pipes, code spans, or escapes independently.
function splitTableRow(line) {
    const source = line.trim();
    const cells = [];
    let cell = "";
    let index = 0;

    while (index < source.length) {
        const character = source[index];

        if (character === "\\") {
            const escapeLength = repeatedLength(source, index, "\\");
            if (source[index + escapeLength] === "|" && escapeLength % 2 === 1) {
                cell += "\\".repeat(Math.floor(escapeLength / 2));
                cell += "|";
                index += escapeLength + 1;
                continue;
            }
            cell += "\\".repeat(escapeLength);
            index += escapeLength;
            continue;
        }

        if (character === "`") {
            const delimiterLength = repeatedLength(source, index, "`");
            const closing = closingCodeDelimiter(source, index + delimiterLength, delimiterLength);
            if (closing !== -1) {
                cell += source.slice(index, closing + delimiterLength);
                index = closing + delimiterLength;
                continue;
            }
        }

        if (character === "|") {
            cells.push(cell.trim());
            cell = "";
            index += 1;
            continue;
        }

        cell += character;
        index += 1;
    }

    cells.push(cell.trim());
    if (source.startsWith("|")) {
        cells.shift();
    }
    if (endsWithUnescapedPipe(source)) {
        cells.pop();
    }
    return cells;
}

function repeatedLength(source, start, character) {
    let end = start;
    while (source[end] === character) {
        end += 1;
    }
    return end - start;
}

function closingCodeDelimiter(source, start, delimiterLength) {
    let index = start;
    while (index < source.length) {
        if (source[index] !== "`") {
            index += 1;
            continue;
        }

        const candidateLength = repeatedLength(source, index, "`");
        if (candidateLength === delimiterLength) {
            return index;
        }
        index += candidateLength;
    }
    return -1;
}

function endsWithUnescapedPipe(source) {
    if (!source.endsWith("|")) {
        return false;
    }

    let backslashes = 0;
    for (let index = source.length - 2; index >= 0 && source[index] === "\\"; index -= 1) {
        backslashes += 1;
    }
    return backslashes % 2 === 0;
}

module.exports = { splitTableRow };
