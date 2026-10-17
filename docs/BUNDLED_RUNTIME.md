# MiniTerm Bash runtime

MiniTerm ships a private PortableGit 2.56.0(2) runtime. The only changed binary
is `usr/bin/msys-2.0.dll`. Installed Git, global PATH and shell startup files
are not replaced. The default profile selects the verified private runtime;
an explicit Bash path still selects the user's chosen installation.

`runtime/BASH_RUNTIME.json` pins the official archive, Cygwin source commit,
Git for Windows package recipe and MiniTerm patch. The official 61 patches
are checked against PKGBUILD before applying the additional console fix.
The fix consumes a resize record before sending SIGWINCH, preventing a
second console reader from consuming the next typed character. Geometry
queries cannot suppress the signal's last-reported-size tracking.

Build on Windows with Python 3.12+, Git and an internet connection:

```
python scripts/build-bash-runtime.py
./scripts/prepare-bash-runtime.ps1 -PatchedDll artifacts/runtime-build/msys-2.0.dll -BuildReceipt artifacts/runtime-build/BUILD_RECEIPT.json -ApplicationDirectory artifacts/bundled-application
```

The SDK and build stay inside `artifacts`. Use two compiler jobs. The build
retains logs, compiler/package versions, full source and a receipt with the
DLL hash. Compiler packages come from the SDK's authenticated package manager;
different toolchain versions need not produce byte-identical DLLs.

Every release provides `MiniTerm-msys-3.6.10-source.tar.gz`, containing the
complete modified Cygwin/newlib source, official recipe and patches, licenses,
MiniTerm patch, build script and receipt. Source is also recoverable from the
pinned public repositories and patch. The source archive's `source` directory
already has all patches applied; to rebuild it, use the recorded SDK, run
`winsup/autogen.sh`, and use the configure/make commands in the build script.
The bootstrap script itself expects the MiniTerm repository layout.

Upstream PortableGit retains its complete license and package-version files.
Its source repositories and package recipes are available through
https://github.com/git-for-windows/git and
https://github.com/git-for-windows/MSYS2-packages, with MSYS2 package sources at
https://repo.msys2.org/msys/sources/ and https://repo.msys2.org/mingw/sources/.
The package includes a source offer and exact package inventory.

Release gates require every bundled Session/ConPTY compatibility check to
pass, default-profile selection, Windows bridge and agent integration checks,
installer option/upgrade/removal checks, and complete checksum verification.
The former installed-Git known-failure allowance is not a release gate.
