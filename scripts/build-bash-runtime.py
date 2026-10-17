"""Build the private MSYS runtime from pinned upstream source; never install it.

Windows/Python 3.12+: python scripts/build-bash-runtime.py [--sdk OWNED_SDK]
The default SDK is created inside artifacts. Logs and source are retained.
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[1]
LOCK = json.loads((ROOT / 'runtime/BASH_RUNTIME.json').read_text(encoding='utf-8'))
PATCH = ROOT / 'docs/patches/msys-resize-signal-after-discard.patch'


def sha(path):
    with Path(path).open('rb') as file:
        return hashlib.file_digest(file, 'sha256').hexdigest()


def git(*args, cwd=None):
    return subprocess.check_output(['git', *map(str, args)], cwd=cwd, text=True).strip()


def clone(url, commit, dest):
    if dest.exists():
        raise RuntimeError(f'Use a fresh build directory: {dest}')
    git('init', dest)
    git('-C', dest, 'config', 'core.autocrlf', 'false')
    git('-C', dest, 'remote', 'add', 'origin', url)
    git('-C', dest, 'fetch', '--depth=1', 'origin', commit)
    git('-C', dest, 'checkout', '--detach', 'FETCH_HEAD')
    if git('-C', dest, 'rev-parse', 'HEAD') != commit:
        raise RuntimeError('Source commit mismatch')


def source_archive(destination, source, recipes, receipt):
    """Full modified runtime source, official recipe, licenses and build controls."""
    with tarfile.open(destination, 'w:gz', compresslevel=6) as archive:
        def include(info):
            parts = Path(info.name).parts
            if '.git' in parts or info.name.endswith(('.orig', '.rej', '.log')):
                return None
            return info
        archive.add(source, arcname='miniterm-msys/source', filter=include)
        archive.add(recipes, arcname='miniterm-msys/official-recipe', filter=include)
        for path, name in [
            (PATCH, 'miniterm.patch'),
            (ROOT / 'runtime/BASH_RUNTIME.json', 'BASH_RUNTIME.json'),
            (ROOT / 'scripts/build-bash-runtime.py', 'build-bash-runtime.py'),
            (ROOT / 'docs/BUNDLED_RUNTIME.md', 'README.md'),
            (receipt, 'BUILD_RECEIPT.json'),
        ]:
            archive.add(path, arcname='miniterm-msys/' + name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sdk', type=Path)
    parser.add_argument('--directory', type=Path, default=ROOT / 'artifacts/runtime-build')
    args = parser.parse_args()
    stage = args.directory.resolve()
    if not stage.is_relative_to(ROOT / 'artifacts') or stage.exists():
        raise RuntimeError('Build requires a fresh directory inside artifacts')
    if sha(PATCH) != LOCK['patchSha256']:
        raise RuntimeError('Runtime patch checksum mismatch')
    stage.mkdir(parents=True)
    sdk = args.sdk.resolve() if args.sdk else stage / 'sdk'
    if not sdk.is_relative_to(ROOT / 'artifacts'):
        raise RuntimeError('Use an isolated SDK inside artifacts; installed Git is never modified')
    if not args.sdk:
        clone('https://github.com/git-for-windows/git-sdk-64', LOCK['sdkCommit'], sdk)
    if git('-C', sdk, 'rev-parse', 'HEAD') != LOCK['sdkCommit']:
        raise RuntimeError('SDK checkout mismatch')
    temporary = stage / 'tmp'
    temporary.mkdir()
    env = dict(os.environ, MSYSTEM='MSYS', TEMP=str(temporary), TMP=str(temporary),
               LC_ALL='C.UTF-8', SOURCE_DATE_EPOCH=str(LOCK['sourceDateEpoch']))
    def run(label, code, *arguments):
        print(label, flush=True)
        with (stage / (label + '.log')).open('wb') as log:
            process = subprocess.Popen(
                [str(sdk / 'usr/bin/bash.exe'), '--noprofile', '--norc', '-c',
                 'set -e; export PATH=/opt/bin:/usr/bin:/c/Windows/System32; ' + code,
                 'miniterm-runtime-build', *map(str, arguments)],
                env=env, cwd=stage, stdout=log, stderr=subprocess.STDOUT,
                creationflags=subprocess.CREATE_NO_WINDOW | subprocess.BELOW_NORMAL_PRIORITY_CLASS)
            if process.wait():
                raise RuntimeError((stage / (label + '.log')).read_text(errors='replace')[-6000:])
    if not args.sdk:
        run('dependencies', 'pacman -Sy --needed --noconfirm cocom mingw-w64-cross-gcc mingw-w64-cross-crt mingw-w64-cross-zlib zlib-devel libzstd-devel gettext-devel libiconv-devel')
    run('toolchain', 'gcc --version; gcc -dumpmachine; pacman -Q')
    # Same locale workaround as the pinned upstream PKGBUILD. Autoconf and
    # Makefiles force LC_ALL=C, even when the parent selected C.UTF-8.
    for name in ('gawk', 'awk'):
        (sdk / 'usr/bin' / name).write_text(
            '#!/bin/sh\nLC_ALL=C.UTF-8 exec ' + name + '.exe "$@"\n',
            encoding='ascii', newline='\n')
    source = stage / 'source'
    recipes = stage / 'recipes'
    clone('https://github.com/cygwin/cygwin', LOCK['msysSourceCommit'], source)
    clone('https://github.com/git-for-windows/MSYS2-packages', LOCK['recipeCommit'], recipes)
    pkg = recipes / 'msys2-runtime'
    recipe = (pkg / 'PKGBUILD').read_text()
    files = ['msys2-runtime.commit'] + sorted(p.name for p in pkg.glob('*.patch'))
    hashes = re.findall(r"'([0-9a-f]{64})'", recipe.split('sha256sums=(', 1)[1].split(')', 1)[0])
    if len(files) != 62 or len(hashes) != 63:
        raise RuntimeError('Unexpected official patch list')
    for name, digest in zip(files, hashes[1:]):
        if sha(pkg / name) != digest:
            raise RuntimeError('Official patch checksum mismatch: ' + name)
    run('prepare', '''src=$(cygpath -u "$1"); pkg=$(cygpath -u "$2")
cd "$src"
git config core.symlinks true
git checkout-index -af
for patchfile in "$pkg"/*.patch; do patch -Nbp1 -i "$patchfile"; done
patch -p1 -i "$(cygpath -u "$3")"
''', source, pkg, PATCH)
    run('autogen', 'cd "$(cygpath -u "$1")/winsup"; ./autogen.sh', source)
    build = stage / 'build'
    build.mkdir()
    run('configure', '''src=$(cygpath -u "$1"); cd "$(cygpath -u "$2")"
export CFLAGS="-O2 -pipe -g0 -DCYGPORT_RELEASE_INFO=3.6.10"
export CXXFLAGS="-O2 -pipe -g0"
"$src/configure" --prefix=/usr --build=x86_64-pc-cygwin --sysconfdir=/etc --disable-doc --disable-dumper --with-msys2-runtime-commit="$3"
''', source, build, LOCK['upstreamRuntimeCommit'])
    run('newlib', 'cd "$(cygpath -u "$1")"; make -j2 all-target-newlib configure-target-winsup', build)
    run('runtime', 'cd "$(cygpath -u "$1")/x86_64-pc-cygwin/winsup/cygwin"; make -j2 new-msys-2.0.dll', build)
    dll = build / 'x86_64-pc-cygwin/winsup/cygwin/new-msys-2.0.dll'
    shutil.copy2(dll, stage / 'msys-2.0.dll')
    receipt = {key: LOCK[key] for key in ('msysSourceCommit', 'recipeCommit', 'sdkCommit', 'patchSha256', 'upstreamRuntimeCommit', 'sourceDateEpoch')}
    receipt.update(schemaVersion=1, msysSha256=sha(dll), builtUtc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                   toolchain=(stage / 'toolchain.log').read_text(errors='replace'))
    receipt_path = stage / 'BUILD_RECEIPT.json'
    receipt_path.write_text(json.dumps(receipt, indent=2), encoding='utf-8')
    source_archive(stage / LOCK['sourceArchive'], source, pkg, receipt_path)
    print(stage, flush=True)


if __name__ == '__main__':
    main()
