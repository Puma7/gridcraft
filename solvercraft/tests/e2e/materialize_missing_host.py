"""Populate absent host files from a pinned auxiliary checkout without replacing edits."""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import shutil
import subprocess

def main() -> int:
    parser=argparse.ArgumentParser()
    parser.add_argument("source",type=Path)
    parser.add_argument("destination",type=Path)
    parser.add_argument("--revision",required=True)
    parser.add_argument("--report",type=Path,required=True)
    args=parser.parse_args()
    source=args.source.resolve()
    destination=args.destination.resolve()
    if not source.is_dir() or not destination.is_dir() or source==destination:
        parser.error("Distinct existing source and destination directories are required")
    actual=subprocess.run(["git","-C",str(source),"rev-parse","HEAD"],text=True,capture_output=True,check=True).stdout.strip()
    if actual!=args.revision:
        parser.error(f"Source revision mismatch: {actual}")
    added,retained=[],[]
    for item in sorted(source.rglob("*")):
        relative=item.relative_to(source)
        if relative.parts[0] in {".git","solvercraft"}:
            continue
        if not item.is_file():
            continue
        target=destination/relative
        if not target.resolve().is_relative_to(destination):
            raise ValueError("Unsafe destination")
        if target.exists():
            retained.append(relative.as_posix())
            continue
        target.parent.mkdir(parents=True,exist_ok=True)
        try:
            with target.open("xb") as output, item.open("rb") as input_file:
                shutil.copyfileobj(input_file,output)
            added.append(relative.as_posix())
        except FileExistsError:
            retained.append(relative.as_posix())
    report={"revision":actual,"mode":"exclusive-create absent host files only; .git and SDK excluded",
            "added_count":len(added),"retained_count":len(retained),
            "added":added,"retained":retained}
    args.report.parent.mkdir(parents=True,exist_ok=True)
    args.report.write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8")
    print(json.dumps({"revision":actual,"added":len(added),"retained":len(retained)}))
    return 0

if __name__=="__main__":
    raise SystemExit(main())

