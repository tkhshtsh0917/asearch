import { seededRandom } from "./statistics.mjs";

export function workload(units = 10000, seed = 31) {
  if (!Number.isSafeInteger(units) || units < 10000 || units > 10000000)
    throw new RangeError("units must be in 10000..10000000");
  const random = seededRandom(seed);
  const cumulative = [];
  let total = 0;
  for (let i = 1; i <= 3000; i++) {
    total += 1 / i ** 1.05;
    cumulative.push(total);
  }
  const symbol = () => {
    const target = random() * total;
    let lo = 0,
      hi = cumulative.length - 1;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if (cumulative[mid] < target) lo = mid + 1;
      else hi = mid;
    }
    return String.fromCharCode(0x4e00 + lo);
  };
  const texts = [];
  let count = 0;
  while (count < units) {
    const length = Math.min(units - count, 10 + Math.floor(random() * 51));
    texts.push(Array.from({ length }, symbol).join(""));
    count += length;
  }
  return {
    seed,
    units,
    texts,
    patterns: [3, 7, 15, 31].map((m) =>
      texts.find((text) => text.length >= m).slice(0, m),
    ),
  };
}
