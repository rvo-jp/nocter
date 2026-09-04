"use strict";

const { MAX_SCALAR, SURROGATE_END, SURROGATE_START } = require("./ucd");

function isScalar(value) {
  return Number.isInteger(value)
    && value >= 0
    && value <= MAX_SCALAR
    && (value < SURROGATE_START || value > SURROGATE_END);
}

function contains(ranges, value) {
  let low = 0;
  let high = ranges.length;
  while (low < high) {
    const middle = low + Math.floor((high - low) / 2);
    const [start, end] = ranges[middle];
    if (value < start) high = middle;
    else if (value > end) low = middle + 1;
    else return true;
  }
  return false;
}

function coverage(ranges) {
  const values = new Uint8Array(MAX_SCALAR + 1);
  for (const [start, end] of ranges) values.fill(1, start, end + 1);
  return values;
}

function validateRanges(name, ranges) {
  let previousEnd = -2;
  let count = 0;
  for (const [start, end] of ranges) {
    if (!isScalar(start) || !isScalar(end) || start > end) {
      throw new Error(`${name} contains an invalid scalar range`);
    }
    if (start <= previousEnd + 1) {
      throw new Error(`${name} ranges are overlapping, adjacent, or unordered`);
    }
    if (start <= SURROGATE_END && end >= SURROGATE_START) {
      throw new Error(`${name} range crosses the surrogate interval`);
    }
    previousEnd = end;
    count += end - start + 1;
  }
  if (count === 0) throw new Error(`${name} has no scalars`);

  let exhaustiveCount = 0;
  for (let value = 0; value <= MAX_SCALAR; value += 1) {
    if (value >= SURROGATE_START && value <= SURROGATE_END) continue;
    if (contains(ranges, value)) exhaustiveCount += 1;
  }
  if (count !== exhaustiveCount) throw new Error(`${name} exhaustive lookup count changed`);
  return count;
}

function mappingAt(mapping, index, direction) {
  const offsets = direction === "lower" ? mapping.lowerOffsets : mapping.upperOffsets;
  const scalars = direction === "lower" ? mapping.lowerScalars : mapping.upperScalars;
  return scalars.slice(offsets[index], offsets[index + 1]);
}

function sameSequence(left, right) {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function validatePackedMapping(name, mapping) {
  if (mapping.lowerOffsets.length !== mapping.points.length + 1
      || mapping.upperOffsets.length !== mapping.points.length + 1) {
    throw new Error(`${name} offset cardinality does not match its key table`);
  }
  let previous = -1;
  for (let index = 0; index < mapping.points.length; index += 1) {
    const point = mapping.points[index];
    if (!isScalar(point) || point <= previous) throw new Error(`${name} keys are invalid or unordered`);
    previous = point;
    for (const [offsets, scalars, direction] of [
      [mapping.lowerOffsets, mapping.lowerScalars, "lower"],
      [mapping.upperOffsets, mapping.upperScalars, "upper"],
    ]) {
      const start = offsets[index];
      const end = offsets[index + 1];
      if (!Number.isInteger(start) || !Number.isInteger(end) || start >= end || end > scalars.length) {
        throw new Error(`${name} ${direction} mapping bounds are invalid for U+${point.toString(16)}`);
      }
      if (!scalars.slice(start, end).every(isScalar)) {
        throw new Error(`${name} ${direction} mapping contains a non-scalar value`);
      }
    }
  }
  if (mapping.lowerOffsets.at(-1) !== mapping.lowerScalars.length
      || mapping.upperOffsets.at(-1) !== mapping.upperScalars.length) {
    throw new Error(`${name} terminal offsets do not consume their scalar tables`);
  }

  let found = 0;
  for (let value = 0; value <= MAX_SCALAR; value += 1) {
    if (value >= SURROGATE_START && value <= SURROGATE_END) continue;
    let low = 0;
    let high = mapping.points.length;
    while (low < high) {
      const middle = low + Math.floor((high - low) / 2);
      if (mapping.points[middle] < value) low = middle + 1;
      else high = middle;
    }
    if (low < mapping.points.length && mapping.points[low] === value) found += 1;
  }
  if (found !== mapping.points.length) throw new Error(`${name} exhaustive key lookup changed`);
}

function validateLowerMapping(name, mapping) {
  if (mapping.offsets.length !== mapping.points.length + 1) {
    throw new Error(`${name} offset cardinality does not match its key table`);
  }
  let previous = -1;
  for (let index = 0; index < mapping.points.length; index += 1) {
    const point = mapping.points[index];
    const start = mapping.offsets[index];
    const end = mapping.offsets[index + 1];
    if (!isScalar(point) || point <= previous || start >= end || end > mapping.scalars.length) {
      throw new Error(`${name} contains an invalid key or mapping bound`);
    }
    if (!mapping.scalars.slice(start, end).every(isScalar)) {
      throw new Error(`${name} contains a non-scalar mapping value`);
    }
    previous = point;
  }
  if (mapping.offsets.at(-1) !== mapping.scalars.length) {
    throw new Error(`${name} terminal offset does not consume its scalar table`);
  }
}

function validateProduct(product, oracle) {
  const counts = {};
  for (const [name, ranges] of Object.entries(product.properties)) {
    counts[name] = validateRanges(name, ranges);
    const expected = coverage(oracle.properties[name]);
    for (let value = 0; value <= MAX_SCALAR; value += 1) {
      if (value >= SURROGATE_START && value <= SURROGATE_END) continue;
      if (contains(ranges, value) !== (expected[value] === 1)) {
        throw new Error(`${name} differs from its pinned corpus at U+${value.toString(16)}`);
      }
    }
  }
  validatePackedMapping("default casing", product.mapping);
  for (let index = 0; index < product.mapping.points.length; index += 1) {
    const point = product.mapping.points[index];
    const expected = oracle.mapping.get(point);
    if (!expected
        || !sameSequence(mappingAt(product.mapping, index, "lower"), expected.lower)
        || !sameSequence(mappingAt(product.mapping, index, "upper"), expected.upper)) {
      throw new Error(`default casing differs from its pinned corpus at U+${point.toString(16)}`);
    }
  }
  if (product.mapping.points.length !== oracle.mapping.size) {
    throw new Error("default casing key count differs from its pinned corpus");
  }
  validateLowerMapping("Final_Sigma casing", product.finalSigma);
  for (let index = 0; index < product.finalSigma.points.length; index += 1) {
    const point = product.finalSigma.points[index];
    const expected = oracle.finalSigma.get(point);
    const actual = product.finalSigma.scalars.slice(
      product.finalSigma.offsets[index],
      product.finalSigma.offsets[index + 1],
    );
    if (!expected || !sameSequence(actual, expected)) {
      throw new Error(`Final_Sigma casing differs from its pinned corpus at U+${point.toString(16)}`);
    }
  }
  if (product.finalSigma.points.length !== oracle.finalSigma.size) {
    throw new Error("Final_Sigma casing key count differs from its pinned corpus");
  }
  return {
    propertyCounts: counts,
    mappingCount: product.mapping.points.length,
    finalSigmaCount: product.finalSigma.points.length,
  };
}

module.exports = { validateProduct };
