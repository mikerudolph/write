import glob
import json
import os
import statistics
import sys


def load(directory, size, editor):
    paths = sorted(glob.glob(os.path.join(directory, f"{size}-{editor}-*.json")))
    return [json.load(open(path)) for path in paths]


def median(values):
    return round(statistics.median(values), 2) if values else None


def typing_metric(run, position):
    entry = run["typing"][position]
    return entry.get("processing_ms", entry["to_frame_ms"])


def summarize(runs):
    if not runs:
        return None
    result = {
        "runs": len(runs),
        "first_paint_ms": median([run["load"]["first_paint_ms"] for run in runs]),
        "first_paint_range_ms": [
            round(min(run["load"]["first_paint_ms"] for run in runs), 1),
            round(max(run["load"]["first_paint_ms"] for run in runs), 1),
        ],
        "memory_after_load_mb": median([run["memory_after_load_mb"] for run in runs]),
        "memory_after_scroll_mb": median([run["memory_after_scroll_mb"] for run in runs]),
        "jump_to_end_ms": median([run["jump_to_end_ms"] for run in runs]),
        "scroll_p50_ms": median([run["scroll"]["frame_ms"]["p50"] for run in runs]),
        "scroll_p99_ms": median([run["scroll"]["frame_ms"]["p99"] for run in runs]),
        "scroll_max_ms": median([run["scroll"]["frame_ms"]["max"] for run in runs]),
        "scroll_hitches": [run["scroll"]["hitches_over_25ms"] for run in runs],
        "selection": [run["selection"] for run in runs][-1],
    }
    for position in ("start", "middle", "end"):
        result[f"typing_{position}"] = {
            key: median([typing_metric(run, position)[key] for run in runs])
            for key in ("p50", "p95", "p99", "max")
        }
        result[f"typing_{position}"]["inserted"] = [
            run["typing"][position]["inserted"] for run in runs
        ]
    return result


def main():
    directory = sys.argv[1]
    output = {}
    for size in ("1m", "5m"):
        for editor in ("textkit", "web"):
            output[f"{size}-{editor}"] = summarize(load(directory, size, editor))
    print(json.dumps(output, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
