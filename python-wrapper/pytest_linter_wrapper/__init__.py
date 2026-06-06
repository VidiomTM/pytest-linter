import hashlib
import os
import platform
import stat
import subprocess
import sys
import urllib.request
from pathlib import Path

REPO = "Jonathangadeaharder/pytest-linter"
VERSION = "0.1.0"
BIN_NAME = "pytest-linter"


def _get_platform_asset_name() -> str:
    system = platform.system().lower()
    machine = platform.machine().lower()

    if system == "linux":
        if machine in ("x86_64", "amd64"):
            return f"{BIN_NAME}-x86_64-unknown-linux-gnu.tar.gz"
        if machine in ("aarch64", "arm64"):
            return f"{BIN_NAME}-aarch64-unknown-linux-gnu.tar.gz"
    elif system == "darwin":
        if machine in ("x86_64", "amd64"):
            return f"{BIN_NAME}-x86_64-apple-darwin.tar.gz"
        if machine in ("aarch64", "arm64"):
            return f"{BIN_NAME}-aarch64-apple-darwin.tar.gz"
    elif system == "windows":
        if machine in ("x86_64", "amd64"):
            return f"{BIN_NAME}-x86_64-pc-windows-msvc.exe.zip"
        if machine in ("aarch64", "arm64"):
            raise RuntimeError(
                f"Windows ARM64 is not yet supported. "
                f"Use an x86_64 Python installation or install manually from "
                f"https://github.com/{REPO}/releases"
            )
        raise RuntimeError(
            f"Unsupported Windows architecture: {machine}. "
            f"Supported: x86_64/amd64. "
            f"Install manually from https://github.com/{REPO}/releases"
        )

    raise RuntimeError(f"Unsupported platform: {system}-{machine}")


def _get_bin_dir() -> Path:
    system = platform.system()
    if system == "Windows":
        base = Path(os.environ.get("LOCALAPPDATA", Path.home() / "AppData" / "Local"))
        return base / "pytest-linter" / "bin" / VERSION
    base = Path(os.environ.get("XDG_BIN_HOME", Path.home() / ".local" / "bin"))
    return base / "pytest-linter" / VERSION


def _download(url: str, dest: Path) -> None:
    print(f"Downloading {url}")
    urllib.request.urlretrieve(url, dest)


def _verify_checksum(filepath: Path, checksum_url: str) -> None:
    try:
        checksum_data = urllib.request.urlopen(checksum_url).read().decode().strip()
    except Exception as exc:
        raise RuntimeError(f"Failed to fetch checksum from {checksum_url}") from exc

    first_line = checksum_data.splitlines()[0].strip()
    parts = first_line.split(None, 1)
    if not parts:
        raise RuntimeError(f"Invalid checksum format: {first_line!r}")
    expected_hash = parts[0]
    if len(parts) > 1:
        checksum_filename = parts[1].lstrip("*")
        if checksum_filename != filepath.name:
            raise RuntimeError(f"Checksum filename mismatch: expected {filepath.name!r}, got {checksum_filename!r}")

    sha256 = hashlib.sha256(filepath.read_bytes()).hexdigest()
    if sha256 != expected_hash:
        raise RuntimeError(f"Checksum mismatch for {filepath}")


def install_binary() -> Path:
    bin_dir = _get_bin_dir()
    bin_dir.mkdir(parents=True, exist_ok=True)

    binary_name = "pytest-linter.exe" if platform.system() == "Windows" else "pytest-linter"
    binary_path = bin_dir / binary_name

    version_marker = bin_dir / ".version"
    if binary_path.exists() and version_marker.exists() and version_marker.read_text().strip() == VERSION:
        return binary_path

    asset_name = _get_platform_asset_name()
    base_url = f"https://github.com/{REPO}/releases/download/v{VERSION}"
    download_url = f"{base_url}/{asset_name}"
    checksum_url = f"{download_url}.sha256"

    archive_path = bin_dir / asset_name
    _download(download_url, archive_path)
    _verify_checksum(archive_path, checksum_url)

    import tarfile
    import zipfile

    if asset_name.endswith(".tar.gz"):
        with tarfile.open(archive_path, "r:gz") as tar:
            tar.extractall(bin_dir)
    elif asset_name.endswith(".zip"):
        with zipfile.ZipFile(archive_path, "r") as zf:
            zf.extractall(bin_dir)

    archive_path.unlink()

    if platform.system() != "Windows":
        binary_path.chmod(binary_path.stat().st_mode | stat.S_IEXEC)

    (bin_dir / ".version").write_text(VERSION + "\n")

    return binary_path


def main() -> None:
    binary = install_binary()
    result = subprocess.run([str(binary)] + sys.argv[1:], check=False)
    sys.exit(result.returncode)


if __name__ == "__main__":
    main()
