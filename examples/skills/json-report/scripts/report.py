import json
import sys

values = json.loads(sys.argv[1])
if not isinstance(values, list) or len(values) > 10000:
    raise ValueError("Expected an array of at most 10000 numbers")
if any(type(value) not in (int, float) for value in values):
    raise ValueError("All values must be numbers")
total = sum(values)
print(json.dumps({"count": len(values), "sum": total, "mean": total / len(values) if values else None}, allow_nan=False))
