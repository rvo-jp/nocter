// Builds a presentation-only declaration outline from published Nocter source. This index never
// validates syntax or decides language semantics; the compiler remains the authority for both.
// It recognizes declaration headers conservatively so documentation navigation and search can
// address source that has already passed the repository's compiler checks.

function indexNocterSource(source) {
    const lines = String(source).replace(/\r/g, "").split("\n");
    const symbols = [];
    const identifiers = new Map();
    const scopes = [];
    let braceDepth = 0;
    let documentation = [];

    lines.forEach((line, index) => {
        const trimmed = line.trim();
        while (scopes.length > 0 && braceDepth < scopes.at(-1).depth) scopes.pop();

        if (trimmed.startsWith("///")) {
            documentation.push(trimmed.slice(3).trim());
            return;
        }
        if (!trimmed || trimmed.startsWith("//")) {
            if (!trimmed) documentation = [];
            return;
        }

        const parent = scopes.at(-1) || null;
        const declaration = recognizeDeclaration(trimmed, parent);
        if (declaration) {
            const qualifiedName = declaration.member && parent
                ? `${parent.owner}.${declaration.name}`
                : declaration.name;
            const id = uniqueIdentifier(
                `declaration-${slugify(`${declaration.kind}-${qualifiedName}`)}`,
                identifiers
            );
            const symbol = Object.freeze({
                id,
                line: index + 1,
                level: declaration.member ? 3 : 2,
                kind: declaration.kind,
                name: declaration.name,
                qualifiedName,
                label: declaration.label,
                documentation: documentation.join(" ")
            });
            symbols.push(symbol);

            const delta = braceDelta(line);
            if (declaration.scope && delta > 0) {
                scopes.push({ owner: declaration.owner, kind: declaration.kind, depth: braceDepth + delta });
            }
        }

        documentation = [];
        braceDepth += braceDelta(line);
        while (scopes.length > 0 && braceDepth < scopes.at(-1).depth) scopes.pop();
    });

    return Object.freeze(symbols);
}

function recognizeDeclaration(line, parent) {
    const nominal = line.match(/^(?:pub\s+)?(?:(?:primitive|copy)\s+)?(struct|enum|interface)\s+([A-Za-z_][A-Za-z0-9_]*)/);
    if (nominal) {
        return scopedDeclaration(nominal[1], nominal[2], `${nominal[1]} ${nominal[2]}`);
    }

    const typeScope = line.match(/^(construct|instance)\s+(.+?)(?=\s+where\b|\s*\{|$)/)
        || line.match(/^(drop)\s+([A-Za-z_][A-Za-z0-9_]*(?:<[^>]+>)?)/);
    if (typeScope) {
        const owner = baseTypeName(typeScope[2]);
        return scopedDeclaration(typeScope[1], owner, `${typeScope[1]} ${typeScope[2]}`);
    }

    const method = line.match(/^(?:pub\s+)?(?:(?:noalloc|blocking|async|default)\s+)*method\s+.+?\.([A-Za-z_][A-Za-z0-9_]*)\s*(?:<[^>]+>)?\s*\(/);
    if (method) return memberDeclaration("method", method[1], `${method[1]}()`);

    const function_ = line.match(/^(?:pub\s+)?(?:primitive\s+)?(?:(?:noalloc|blocking|async)\s+)*func\s+([A-Za-z_][A-Za-z0-9_]*)/);
    if (function_) {
        const declaration = memberDeclaration("func", function_[1], `${function_[1]}()`);
        return parent ? declaration : { ...declaration, member: false };
    }

    const constant = line.match(/^(?:pub\s+)?const\s+([A-Za-z_][A-Za-z0-9_]*)/);
    if (constant) {
        const declaration = memberDeclaration("const", constant[1], constant[1]);
        return parent ? declaration : { ...declaration, member: false };
    }

    const associatedType = line.match(/^(?:pub\s+)?(?:primitive\s+)?type\s+([A-Za-z_][A-Za-z0-9_]*)/);
    if (associatedType) {
        const declaration = memberDeclaration("type", associatedType[1], associatedType[1]);
        return parent ? declaration : { ...declaration, member: false };
    }

    const literal = line.match(/^(?:pub\s+)?(?:default\s+)?literal\s+([^\s(]+)/);
    if (literal) return memberDeclaration("literal", literal[1], `literal ${literal[1]}`);

    if (/^(?:pub\s+)?(?:(?:noalloc|blocking|async)\s+)*coerce\b/.test(line)) {
        return memberDeclaration("coerce", "coerce", "coerce");
    }
    if (/^(?:pub\s+)?(?:(?:noalloc|blocking|async)\s+)*operator\b/.test(line)) {
        return memberDeclaration("operator", "operator", "operator");
    }

    const implementation = line.match(/^impl\s+([A-Za-z_][A-Za-z0-9_.]*)/);
    if (implementation) return memberDeclaration("impl", implementation[1], `impl ${implementation[1]}`);

    const field = parent && line.match(/^pub\s+([A-Za-z_][A-Za-z0-9_]*)\s*:/);
    if (field) return memberDeclaration("field", field[1], field[1]);

    const variant = parent?.kind === "enum" && line.match(/^([A-Za-z_][A-Za-z0-9_]*)\b/);
    if (variant) return memberDeclaration("variant", variant[1], variant[1]);

    return null;
}

function scopedDeclaration(kind, owner, label) {
    return { kind, name: owner, owner, label, member: false, scope: true };
}

function memberDeclaration(kind, name, label) {
    return { kind, name, owner: name, label, member: true, scope: false };
}

function baseTypeName(type) {
    return type.replace(/<.*$/, "");
}

function braceDelta(line) {
    let delta = 0;
    let quoted = null;
    let escaped = false;

    for (let index = 0; index < line.length; index += 1) {
        const character = line[index];
        const next = line[index + 1];
        if (!quoted && character === "/" && next === "/") break;
        if (quoted) {
            if (escaped) escaped = false;
            else if (character === "\\") escaped = true;
            else if (character === quoted) quoted = null;
            continue;
        }
        if (character === '"' || character === "'") quoted = character;
        else if (character === "{") delta += 1;
        else if (character === "}") delta -= 1;
    }

    return delta;
}

function uniqueIdentifier(base, identifiers) {
    const count = identifiers.get(base) || 0;
    identifiers.set(base, count + 1);
    return count === 0 ? base : `${base}-${count + 1}`;
}

function slugify(value) {
    return String(value)
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-+|-+$/g, "") || "symbol";
}

module.exports = { indexNocterSource };
