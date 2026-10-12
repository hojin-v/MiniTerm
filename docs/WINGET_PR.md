## Description

New package: `Hnode.MiniTerm` version `0.1.0-alpha.6`.

MiniTerm is an open-source native Bash terminal HUD for Windows 11 x64. This is a tester alpha; its package descriptions and publisher release notes disclose the known resize/input and performance limitations.

The manifest uses the publisher's immutable GitHub Release NSIS installer, user scope, `/S`, the actual Apps & Features publisher/name/version/product key, and package dependencies on `Git.Git` and `Microsoft.VCRedist.2015+.x64`. Installer exit codes preserve running-session and missing-runtime failures.

Publisher release: https://github.com/hojin-v/MiniTerm/releases/tag/v0.1.0-alpha.6

## Checklist

- [ ] Signed the [Contributor License Agreement](https://cla.opensource.microsoft.com)
- [ ] Linked to an issue (if applicable)

## Manifest Checklist

- [ ] Checked that there aren't other open pull requests for the same package/version
- [x] This PR only modifies one package version
- [x] Validated locally with `winget validate --manifest <path>`
- [ ] Tested with `winget install --manifest <path>` in an isolated environment
- [x] Manifest conforms to schema 1.12.0
