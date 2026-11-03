"""Compare scan windows: do advertised fields rotate between receiver restarts?

Usage: analyze.py window1.jsonl window2.jsonl [...]
Each file = one window captured after a receiver restart (new charge/session).
"""
import json
import sys

FIELDS = ["service_data", "peripheral_id", "name", "manufacturer_data", "tx_power"]


def load(path):
  with open(path) as f:
    return [json.loads(line) for line in f if line.strip()]


def key(value):
  return json.dumps(value, sort_keys=True)


def field_values(records, field):
  """value -> (first_ts, last_ts, count); None values are skipped."""
  seen = {}
  for r in records:
    v = r.get(field)
    if v is None:
      continue
    k = key(v)
    first, last, n = seen.get(k, (r["ts"], r["ts"], 0))
    seen[k] = (min(first, r["ts"]), max(last, r["ts"]), n + 1)
  return seen


def analyze_field(windows, field):
  per_window = [field_values(w, field) for w in windows]
  sets = [set(p) for p in per_window]
  shared = set.intersection(*sets) if sets and all(sets) else set()
  repeated = {v for i, a in enumerate(sets) for b in sets[i + 1 :] for v in a & b}
  spans = [last - first for p in per_window for first, last, _ in p.values()]
  distinct = [len(s) for s in sets]
  if len(windows) < 2:
    verdict = "INCONCLUSIVE (need >= 2 windows)"
  elif not any(sets):
    verdict = "ABSENT (never advertised)"
  elif repeated:
    verdict = "STABLE: value reused across windows, usable to track"
  else:
    verdict = "ROTATES: no value reused across windows"
  return {
    "field": field,
    "distinct_per_window": distinct,
    "reused_across_windows": len(repeated),
    "all_windows_shared": len(shared),
    "max_constant_s": round(max(spans), 1) if spans else None,
    "verdict": verdict,
  }


def analyze(windows):
  return [analyze_field(windows, f) for f in FIELDS]


def render(rows):
  header = ["field", "distinct/window", "reused", "max constant s", "verdict"]
  lines = [" | ".join(header), " | ".join("---" for _ in header)]
  for r in rows:
    lines.append(
      f"{r['field']} | {r['distinct_per_window']} | {r['reused_across_windows']} "
      f"| {r['max_constant_s']} | {r['verdict']}"
    )
  return "\n".join(lines)


if __name__ == "__main__":
  paths = sys.argv[1:]
  if len(paths) < 1:
    sys.exit("usage: analyze.py window1.jsonl [window2.jsonl ...]")
  print(render(analyze([load(p) for p in paths])))
