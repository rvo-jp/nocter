"use strict";

const MAX_SCALAR = 0x10ffff;
const SURROGATE_START = 0xd800;
const SURROGATE_END = 0xdfff;

function scalar(value, context) {
  const parsed = Number.parseInt(value, 16);
  if (!/^[0-9A-Fa-f]{4,6}$/.test(value) || parsed > MAX_SCALAR) {
    throw new Error(`${context} contains invalid scalar ${JSON.stringify(value)}`);
  }
  return parsed;
}

function scalarSequence(value, context) {
  const trimmed = value.trim();
  if (trimmed.length === 0) return [];
  return trimmed.split(/\s+/).map((item) => scalar(item, context));
}

function sourceLines(text) {
  return text.split(/\r?\n/).map((line, index) => ({
    number: index + 1,
    value: line.split("#", 1)[0].trim(),
  })).filter((line) => line.value.length !== 0);
}

function parseRange(value, context) {
  const parts = value.trim().split("..");
  if (parts.length === 1) {
    const point = scalar(parts[0], context);
    return [point, point];
  }
  if (parts.length !== 2) throw new Error(`${context} has malformed range`);
  const start = scalar(parts[0], context);
  const end = scalar(parts[1], context);
  if (start > end) throw new Error(`${context} has descending range`);
  return [start, end];
}

function parseProperties(text, requested, fileName) {
  const properties = new Map([...requested].map((name) => [name, []]));
  for (const line of sourceLines(text)) {
    const fields = line.value.split(";");
    if (fields.length < 2) throw new Error(`${fileName}:${line.number} is malformed`);
    const name = fields[1].trim();
    if (!properties.has(name)) continue;
    properties.get(name).push(parseRange(fields[0], `${fileName}:${line.number}`));
  }
  for (const name of requested) {
    if (properties.get(name).length === 0) throw new Error(`${fileName} has no ${name} records`);
  }
  return properties;
}

function mergeRanges(ranges) {
  const sorted = [...ranges].sort((left, right) => left[0] - right[0] || left[1] - right[1]);
  const merged = [];
  for (const [start, end] of sorted) {
    const previous = merged[merged.length - 1];
    if (previous && start <= previous[1] + 1) {
      previous[1] = Math.max(previous[1], end);
    } else {
      merged.push([start, end]);
    }
  }
  return merged;
}

function sameSequence(left, right) {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function parseUnicodeData(text) {
  const decimalRanges = [];
  const simple = new Map();
  let openRange = null;
  let previous = -1;
  for (const line of sourceLines(text)) {
    const fields = line.value.split(";");
    if (fields.length !== 15) throw new Error(`UnicodeData.txt:${line.number} must have 15 fields`);
    const point = scalar(fields[0], `UnicodeData.txt:${line.number}`);
    if (point <= previous) throw new Error(`UnicodeData.txt:${line.number} is not strictly ordered`);
    previous = point;
    const name = fields[1];
    if (name.endsWith(", First>")) {
      if (openRange) throw new Error(`UnicodeData.txt:${line.number} nests ranges`);
      openRange = { start: point, category: fields[2], line: line.number };
      continue;
    }
    if (name.endsWith(", Last>")) {
      if (!openRange || openRange.category !== fields[2]) {
        throw new Error(`UnicodeData.txt:${line.number} does not close its range`);
      }
      if (openRange.category === "Nd") decimalRanges.push([openRange.start, point]);
      openRange = null;
      continue;
    }
    if (openRange) throw new Error(`UnicodeData.txt:${line.number} interrupts a range`);
    if (fields[2] === "Nd") decimalRanges.push([point, point]);
    const upper = fields[12].length === 0 ? [point] : [scalar(fields[12], `UnicodeData.txt:${line.number}`)];
    const lower = fields[13].length === 0 ? [point] : [scalar(fields[13], `UnicodeData.txt:${line.number}`)];
    if (!sameSequence(lower, [point]) || !sameSequence(upper, [point])) {
      simple.set(point, { lower, upper });
    }
  }
  if (openRange) throw new Error(`UnicodeData.txt:${openRange.line} has no Last record`);
  return { decimalRanges, simple };
}

function parseSpecialCasing(text) {
  const unconditional = new Map();
  const finalSigma = new Map();
  const localeTags = new Set(["az", "lt", "tr"]);
  for (const line of sourceLines(text)) {
    const fields = line.value.split(";");
    if ((fields.length !== 5 && fields.length !== 6)
        || (fields.length === 6 && fields[5].trim().length !== 0)) {
      throw new Error(`SpecialCasing.txt:${line.number} has malformed semicolon fields`);
    }
    const point = scalar(fields[0].trim(), `SpecialCasing.txt:${line.number}`);
    const record = {
      lower: scalarSequence(fields[1], `SpecialCasing.txt:${line.number}`),
      upper: scalarSequence(fields[3], `SpecialCasing.txt:${line.number}`),
    };
    const conditions = fields[4].trim().split(/\s+/).filter(Boolean);
    if (conditions.length === 0) {
      if (unconditional.has(point)) throw new Error(`duplicate unconditional casing for U+${point.toString(16)}`);
      unconditional.set(point, record);
      continue;
    }
    if (conditions.some((condition) => localeTags.has(condition))) continue;
    if (conditions.length !== 1 || conditions[0] !== "Final_Sigma") {
      throw new Error(`unsupported locale-independent casing condition ${conditions.join(" ")}`);
    }
    if (finalSigma.has(point)) throw new Error(`duplicate Final_Sigma casing for U+${point.toString(16)}`);
    finalSigma.set(point, record);
  }
  if (finalSigma.size === 0) throw new Error("SpecialCasing.txt has no Final_Sigma rule");
  return { unconditional, finalSigma };
}

function buildMapping(simple, unconditional) {
  const points = [...new Set([...simple.keys(), ...unconditional.keys()])].sort((a, b) => a - b);
  const selected = [];
  for (const point of points) {
    const base = simple.get(point) || { lower: [point], upper: [point] };
    const mapping = unconditional.get(point) || base;
    if (!sameSequence(mapping.lower, [point]) || !sameSequence(mapping.upper, [point])) {
      selected.push({ point, lower: mapping.lower, upper: mapping.upper });
    }
  }
  return {
    packed: packMappings(selected),
    expected: new Map(selected.map((record) => [record.point, record])),
  };
}

function packMappings(records) {
  const points = [];
  const lowerOffsets = [0];
  const lowerScalars = [];
  const upperOffsets = [0];
  const upperScalars = [];
  for (const record of records) {
    if (record.lower.length === 0 || record.upper.length === 0) {
      throw new Error(`default casing for U+${record.point.toString(16)} is empty`);
    }
    points.push(record.point);
    lowerScalars.push(...record.lower);
    upperScalars.push(...record.upper);
    lowerOffsets.push(lowerScalars.length);
    upperOffsets.push(upperScalars.length);
  }
  return { points, lowerOffsets, lowerScalars, upperOffsets, upperScalars };
}

function packLowerMappings(records) {
  const points = [];
  const offsets = [0];
  const scalars = [];
  for (const record of records) {
    if (record.lower.length === 0) {
      throw new Error(`conditional lowercase mapping for U+${record.point.toString(16)} is empty`);
    }
    points.push(record.point);
    scalars.push(...record.lower);
    offsets.push(scalars.length);
  }
  return { points, offsets, scalars };
}

function buildCorpus(inputs) {
  const derived = parseProperties(
    inputs.get("derivedCoreProperties"),
    new Set(["Alphabetic", "Lowercase", "Uppercase", "Cased", "Case_Ignorable"]),
    "DerivedCoreProperties.txt",
  );
  const listed = parseProperties(inputs.get("propertyList"), new Set(["White_Space"]), "PropList.txt");
  const unicode = parseUnicodeData(inputs.get("unicodeData"));
  const special = parseSpecialCasing(inputs.get("specialCasing"));
  const defaultMapping = buildMapping(unicode.simple, special.unconditional);
  const finalSigmaRecords = [...special.finalSigma.entries()]
    .sort((left, right) => left[0] - right[0])
    .map(([point, mapping]) => ({ point, ...mapping }));
  for (const record of finalSigmaRecords) {
    const ordinary = defaultMapping.expected.get(record.point) || {
      point: record.point,
      lower: [record.point],
      upper: [record.point],
    };
    if (!sameSequence(record.upper, ordinary.upper)) {
      throw new Error(`Final_Sigma changes unsupported uppercase mapping for U+${record.point.toString(16)}`);
    }
  }
  const rawProperties = {
    whitespace: listed.get("White_Space"),
    alphabetic: derived.get("Alphabetic"),
    lowercase: derived.get("Lowercase"),
    uppercase: derived.get("Uppercase"),
    decimalDigit: unicode.decimalRanges,
    cased: derived.get("Cased"),
    caseIgnorable: derived.get("Case_Ignorable"),
  };
  const product = {
    properties: {
      whitespace: mergeRanges(rawProperties.whitespace),
      alphabetic: mergeRanges(rawProperties.alphabetic),
      lowercase: mergeRanges(rawProperties.lowercase),
      uppercase: mergeRanges(rawProperties.uppercase),
      decimalDigit: mergeRanges(rawProperties.decimalDigit),
      cased: mergeRanges(rawProperties.cased),
      caseIgnorable: mergeRanges(rawProperties.caseIgnorable),
    },
    mapping: defaultMapping.packed,
    finalSigma: packLowerMappings(finalSigmaRecords),
  };
  return {
    product,
    oracle: {
      properties: rawProperties,
      mapping: defaultMapping.expected,
      finalSigma: new Map(finalSigmaRecords.map((record) => [record.point, record.lower])),
    },
  };
}

module.exports = { MAX_SCALAR, SURROGATE_END, SURROGATE_START, buildCorpus };
