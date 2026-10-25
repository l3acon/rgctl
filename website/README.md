# Website redirect stub

The full marketing site and guides live in **[rgctl-website](https://github.com/rgctl/rgctl-website)** at **https://rgctl.io/**.

This directory is a **GitHub Pages redirect** for the legacy project site
`https://rgctl.io/` (and `*.github.io/rgctl/`):

| File | Role |
|------|------|
| `index.html` | Redirect `/rgctl/` → `https://rgctl.io/` (path-preserving) |
| `404.html` | Same redirect for deep links GitHub Pages cannot resolve |
| `install.sh` | Copied from `scripts/install.sh` so `curl …/install.sh` keeps working |

Deploy: `.github/workflows/website.yml` uploads this folder as the Pages artifact.
