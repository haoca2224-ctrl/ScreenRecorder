import json
import pathlib
import shutil
import subprocess

root = pathlib.Path(__file__).resolve().parents[1]
target = root / "dist" / "ScreenRecorder" / "licenses"
target.mkdir(parents=True, exist_ok=True)
data = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--locked", "--format-version", "1",
     "--filter-platform", "x86_64-pc-windows-msvc"], cwd=root))
inventory = []
for package in data["packages"]:
    if package["source"] is None:
        continue
    base = pathlib.Path(package["manifest_path"]).parent
    name = f'{package["name"]}-{package["version"]}'
    dest = target / name
    dest.mkdir(exist_ok=True)
    for file in base.iterdir():
        if file.is_file() and (file.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE"))):
            shutil.copy2(file, dest / file.name)
    inventory.append({key: package.get(key) for key in
                      ("name", "version", "license", "license_file", "repository", "source")})
(target / "packages.json").write_text(json.dumps(inventory, indent=2), encoding="utf-8")
print(f"Collected metadata and license files for {len(inventory)} packages.")
