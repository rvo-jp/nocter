"use strict";

function flatRanges(ranges) {
  return ranges.flatMap(([start, end]) => [start, end]);
}

function renderValues(values, format) {
  const width = 12;
  const lines = [];
  for (let index = 0; index < values.length; index += width) {
    lines.push(`    ${values.slice(index, index + width).map(format).join(", ")},`);
  }
  return lines.join("\n");
}

function hex(value) {
  return `0x${value.toString(16).toUpperCase().padStart(4, "0")}`;
}

function renderStatic(name, type, values, format) {
  return `const ${name}_LEN: usize = ${values.length}\n\nstatic ${name}: [${type}; ${values.length}] = [\n${renderValues(values, format)}\n]\n`;
}

function renderRanges(name, ranges) {
  return renderStatic(`${name}_RANGES`, "u32", flatRanges(ranges), hex);
}

function renderMapping(prefix, mapping) {
  return [
    renderStatic(`${prefix}_POINTS`, "u32", mapping.points, hex),
    renderStatic(`${prefix}_LOWER_OFFSETS`, "usize", mapping.lowerOffsets, String),
    renderStatic(`${prefix}_LOWER_SCALARS`, "u32", mapping.lowerScalars, hex),
    renderStatic(`${prefix}_UPPER_OFFSETS`, "usize", mapping.upperOffsets, String),
    renderStatic(`${prefix}_UPPER_SCALARS`, "u32", mapping.upperScalars, hex),
  ].join("\n");
}

function renderLowerMapping(prefix, mapping) {
  return [
    renderStatic(`${prefix}_POINTS`, "u32", mapping.points, hex),
    renderStatic(`${prefix}_OFFSETS`, "usize", mapping.offsets, String),
    renderStatic(`${prefix}_SCALARS`, "u32", mapping.scalars, hex),
  ].join("\n");
}

function renderProduct(version, product, summary) {
  const counts = Object.entries(summary.propertyCounts)
    .map(([name, count]) => `// ${name}: ${count} scalars`)
    .join("\n");
  return `//! Generated Unicode ${version} data. Do not edit.\n//! Regenerate with node development/unicode/generate.js --write.\n\n${counts}\n// default casing: ${summary.mappingCount} mapped scalars\n// Final_Sigma casing: ${summary.finalSigmaCount} conditional scalars\n\n${renderRanges("WHITE_SPACE", product.properties.whitespace)}\n${renderRanges("ALPHABETIC", product.properties.alphabetic)}\n${renderRanges("LOWERCASE", product.properties.lowercase)}\n${renderRanges("UPPERCASE", product.properties.uppercase)}\n${renderRanges("DECIMAL_DIGIT", product.properties.decimalDigit)}\n${renderRanges("CASED", product.properties.cased)}\n${renderRanges("CASE_IGNORABLE", product.properties.caseIgnorable)}\n${renderMapping("CASE", product.mapping)}\n${renderLowerMapping("FINAL_SIGMA_LOWER", product.finalSigma)}`;
}

module.exports = { renderProduct };
