# AUR Publishing Workflow

This directory holds the PKGBUILD for the `larkline-bin` AUR package. The AUR
repository lives at `ssh://aur@aur.archlinux.org/larkline-bin.git` — separate
from this repo, same content.

## First-time publish

1. Create the package on AUR (needs a registered AUR account + SSH key).
2. Clone the empty AUR repo somewhere:
   ```
   git clone ssh://aur@aur.archlinux.org/larkline-bin.git /tmp/aur-larkline-bin
   cp packaging/aur/larkline-bin/PKGBUILD /tmp/aur-larkline-bin/
   cd /tmp/aur-larkline-bin
   ```
3. Generate checksums and the `.SRCINFO`:
   ```
   updpkgsums
   makepkg --printsrcinfo > .SRCINFO
   ```
4. Commit both files and push:
   ```
   git add PKGBUILD .SRCINFO
   git commit -m "Initial import (0.10.0)"
   git push
   ```

## Subsequent releases

1. Bump `pkgver` in `packaging/aur/larkline-bin/PKGBUILD` to match the new
   tag (same commit that updates `Cargo.toml`, or as a follow-up).
2. In the AUR clone, refresh:
   ```
   cp packaging/aur/larkline-bin/PKGBUILD /tmp/aur-larkline-bin/
   cd /tmp/aur-larkline-bin
   updpkgsums
   makepkg --printsrcinfo > .SRCINFO
   git add -A && git commit -m "Release <version>"
   git push
   ```

## Verifying locally

```
cd packaging/aur/larkline-bin
updpkgsums        # fill in sha256sums from the GitHub Release tarball
makepkg -s        # build and install into ./pkg for inspection
```

Not automated via CI for the first release — the SSH identity for AUR is
personal to the maintainer. If churn becomes annoying, wire a GitHub Action
with a dedicated deploy key.
