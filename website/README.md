# Website redirect stub

The full marketing site and guides live in **[rgctl-website](https://github.com/rgctl/rgctl-website)** at **https://rgctl.io/**.

This directory is a **GitHub Pages redirect** for the legacy project site
`https://sshaaf.dev/rgctl/` (as well as `https://shaaf.dev/rgctl/` and `*.github.io/rgctl/`):

| File | Role |
|------|------|
| `index.html` | Redirect `/rgctl/` → `https://rgctl.io/` (path-, query-, and hash-preserving) |
| `404.html` | Same redirect for deep links GitHub Pages cannot resolve directly |
| `install.sh` | Copied from `scripts/install.sh` at build time so `curl …/install.sh` keeps working |

Deploy: [`.github/workflows/website.yml`](../.github/workflows/website.yml) uploads this folder as the GitHub Pages artifact.
