#!/usr/bin/env python3
"""Builds the MCP bundles (.mcpb), one per platform, from the release packages: a zip with
manifest.json, the icon, LICENSE and the statscraft program under server/, which Claude Desktop
installs with a double click. The tool list in the manifest comes from the server itself, so it
always matches. From the repository root, after the other packages are built:
  python3 packaging/mcpb/build.py 0.1.0 [FOLDER]
FOLDER holds the packages (default target/packages); the bundles are written next to them. A
platform whose package is missing is skipped; the Linux package is needed, because its program
gives the tool list, so the script runs on Linux x86_64. Needs only Python 3."""

import json
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path

version = sys.argv[1]
folder = Path(sys.argv[2] if len(sys.argv) > 2 else "target/packages")

# bundle suffix: (package, path of the program inside it, program name in the bundle, platform)
PLATFORMS = {
    "windows-x86_64": (
        f"statscraft-{version}-windows-x86_64.zip",
        f"statscraft-{version}-windows-x86_64/statscraft.exe",
        "statscraft.exe",
        "win32",
    ),
    "macos-universal": (
        f"statscraft-{version}-macos-universal.tar.gz",
        f"statscraft-{version}-macos-universal/statscraft",
        "statscraft",
        "darwin",
    ),
    "linux-x86_64": (
        f"statscraft-{version}-linux-x86_64.tar.gz",
        f"statscraft-{version}-linux-x86_64/bin/statscraft",
        "statscraft",
        "linux",
    ),
}


def program(package: Path, member: str) -> bytes:
    if package.suffix == ".zip":
        with zipfile.ZipFile(package) as z:
            return z.read(member)
    with tarfile.open(package) as t:
        return t.extractfile(member).read()


def tools(binary: bytes) -> list:
    """The name and description of every tool, from tools/list of the running server."""
    with tempfile.TemporaryDirectory() as tmp:
        exe = Path(tmp) / "statscraft"
        exe.write_bytes(binary)
        exe.chmod(0o755)
        requests = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                "protocolVersion": "2025-11-25", "capabilities": {},
                "clientInfo": {"name": "mcpb-build", "version": version}}},
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
        ]
        stdin = "".join(json.dumps(r) + "\n" for r in requests)
        out = subprocess.run([str(exe), "mcp"], input=stdin, capture_output=True, text=True,
                             timeout=60, check=True).stdout
    for line in out.splitlines():
        reply = json.loads(line)
        if reply.get("id") == 2:
            listed = [{"name": t["name"], "description": " ".join(t["description"].split())}
                      for t in reply["result"]["tools"]]
            assert listed, "the server lists no tools"
            return listed
    sys.exit(f"no tools/list reply from the server:\n{out}")


def add(z: zipfile.ZipFile, name: str, data: bytes, mode: int = 0o644) -> None:
    # Unix permissions in the upper 16 bits, so the program stays executable when unpacked
    info = zipfile.ZipInfo(name, date_time=(2020, 1, 1, 0, 0, 0))
    info.create_system = 3
    info.external_attr = (0o100000 | mode) << 16  # a regular file with these permissions
    info.compress_type = zipfile.ZIP_DEFLATED
    z.writestr(info, data)


linux = PLATFORMS["linux-x86_64"]
if not (folder / linux[0]).exists():
    sys.exit(f"{folder / linux[0]} is missing; its program gives the tool list")
template = json.loads(Path(__file__).with_name("manifest.json").read_text())
template["version"] = version
template["tools"] = tools(program(folder / linux[0], linux[1]))

for suffix, (package, member, name, platform) in PLATFORMS.items():
    if not (folder / package).exists():
        print(f"skipped {suffix}: no {package}")
        continue
    manifest = dict(template, compatibility={"platforms": [platform]})
    manifest["server"] = dict(template["server"], entry_point=f"server/{name}")
    bundle = folder / f"statscraft-{version}-{suffix}.mcpb"
    with zipfile.ZipFile(bundle, "w") as z:
        add(z, "manifest.json", (json.dumps(manifest, indent=2) + "\n").encode())
        add(z, "icon.png", Path("crates/gui/icons/statscraft-256.png").read_bytes())
        add(z, "LICENSE", Path("LICENSE").read_bytes())
        add(z, f"server/{name}", program(folder / package, member), 0o755)
    print(f"built {bundle}")
