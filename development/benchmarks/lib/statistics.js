"use strict";

function quantile(sortedValues, fraction) {
  if (sortedValues.length === 0) return null;
  const position = (sortedValues.length - 1) * fraction;
  const lower = Math.floor(position);
  const upper = Math.ceil(position);
  if (lower === upper) return sortedValues[lower];
  return (
    sortedValues[lower] * (upper - position) +
    sortedValues[upper] * (position - lower)
  );
}

function summarize(values) {
  if (values.length === 0) return null;
  const sortedValues = [...values].sort((left, right) => left - right);
  const rounded = (value) => Math.round(value * 1_000) / 1_000;
  return {
    count: sortedValues.length,
    min: rounded(sortedValues[0]),
    median: rounded(quantile(sortedValues, 0.5)),
    p90: rounded(quantile(sortedValues, 0.9)),
    max: rounded(sortedValues[sortedValues.length - 1]),
  };
}

module.exports = { summarize };
