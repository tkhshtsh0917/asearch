export function quantile(values, q) {
  if (!values.length || values.some((x) => !Number.isFinite(x) || x <= 0))
    throw new RangeError("Expected positive finite timing samples");
  if (!Number.isFinite(q) || q < 0 || q > 1)
    throw new RangeError("Invalid quantile");
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor((sorted.length - 1) * q)];
}

export function seededRandom(seed = 31) {
  return () => {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    return seed / 2 ** 32;
  };
}

// Resample whole paired runs, never individual operations as independent runs.
export function pairedInterval(runs, seed = 31) {
  if (runs.length < 5)
    throw new RangeError("At least five paired runs are required");
  const logs = runs.map(({ baseline, candidate }) =>
    Math.log(quantile(candidate, 0.5) / quantile(baseline, 0.5)),
  );
  const random = seededRandom(seed);
  const bootstrap = Array.from({ length: 10000 }, () => {
    let sum = 0;
    for (let i = 0; i < logs.length; i++)
      sum += logs[Math.floor(random() * logs.length)];
    return Math.exp(sum / logs.length);
  });
  return {
    ratio: Math.exp(logs.reduce((a, b) => a + b, 0) / logs.length),
    lower: quantile(bootstrap, 0.025),
    upper: quantile(bootstrap, 0.975),
  };
}

export function regression(first, confirmation) {
  const interval = pairedInterval(first);
  if (interval.lower <= 1.1) return { status: "unconfirmed", interval };
  if (!confirmation) return { status: "candidate", interval };
  const second = pairedInterval(confirmation, 67);
  return {
    status: second.lower > 1.1 ? "confirmed" : "unconfirmed",
    interval,
    confirmation: second,
  };
}
