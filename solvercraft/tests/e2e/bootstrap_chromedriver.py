"""Install the official ChromeDriver test tool into a task-local directory."""
from __future__ import annotations
import argparse,hashlib,io,json,re,urllib.parse,urllib.request,zipfile
from pathlib import Path

METADATA="https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json"

def main()->int:
    parser=argparse.ArgumentParser()
    parser.add_argument("destination",type=Path)
    args=parser.parse_args()
    destination=args.destination.resolve()
    metadata=json.load(urllib.request.urlopen(METADATA,timeout=30))
    stable=metadata["channels"]["Stable"]
    version=stable["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+\.\d+",version):
        raise ValueError("Unexpected ChromeDriver version")
    url=next(item["url"] for item in stable["downloads"]["chromedriver"] if item["platform"]=="win64")
    parsed=urllib.parse.urlsplit(url)
    if parsed.scheme!="https" or parsed.hostname!="storage.googleapis.com" or not parsed.path.startswith(f"/chrome-for-testing-public/{version}/win64/"):
        raise ValueError("Unexpected official download routing")
    with urllib.request.urlopen(url,timeout=30) as response:
        data=response.read(64*1024*1024+1)
    if len(data)>64*1024*1024:
        raise ValueError("Driver download exceeds budget")
    target=destination/("chromedriver-"+version)
    target.mkdir(parents=True,exist_ok=True)
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        if sum(item.file_size for item in archive.infolist())>128*1024*1024:
            raise ValueError("Driver archive exceeds extraction budget")
        for item in archive.infolist():
            if item.is_dir():
                continue
            path=target/Path(item.filename)
            if not path.resolve().is_relative_to(target):
                raise ValueError("Unsafe driver archive path")
            path.parent.mkdir(parents=True,exist_ok=True)
            content=archive.read(item)
            if path.exists() and path.read_bytes()!=content:
                raise ValueError("Refusing to replace a differing existing driver file")
            if not path.exists():
                with path.open("xb") as output:
                    output.write(content)
    driver=next(target.rglob("chromedriver.exe"))
    record={"version":version,"source":url,"metadata":METADATA,"archive_sha256":hashlib.sha256(data).hexdigest(),
        "driver_sha256":hashlib.sha256(driver.read_bytes()).hexdigest(),"driver":str(driver),
        "license_files":[str(p.relative_to(target)) for p in target.rglob("*") if p.is_file() and ("LICENSE" in p.name or "NOTICE" in p.name)]}
    (target/"tool-record.json").write_text(json.dumps(record,indent=2)+"\n",encoding="utf-8")
    print(json.dumps(record))
    return 0

if __name__=="__main__":
    raise SystemExit(main())

