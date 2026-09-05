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
  return {
    count: sortedValues.length,
    min: sortedValues[0],
    median: quantile(sortedValues, 0.5),
    p90: quantile(sortedValues, 0.9),
    max: sortedValues[sortedValues.length - 1],
  };
}

module.exports = { summarize };
