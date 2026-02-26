# VENDORED SUBTREE — DO NOT EDIT

The `signal/` directory is managed via git subtree sync from bolt-rendezvous.

All files under `signal/` are read-only in this repository:
- `signal/docs/STATE.md` — read-only
- `signal/docs/CHANGELOG.md` — read-only
- All source code under `signal/src/` — read-only

Do not update manually. Changes MUST be made upstream in bolt-rendezvous first.

Update procedure:
```
git subtree pull --prefix=signal <bolt-rendezvous-remote> main --squash
```
