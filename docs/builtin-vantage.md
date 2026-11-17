# Built-in Vantage

`app/builtin/vantage.wasm` is compiled into the client (`app/src/modding/builtin.rs`) and loaded
on every start with every local grant. Set `CINNABAR_NO_BUILTIN=1` to start without it.

Rebuild it from the Vantage sources (`C:\cb\vantage`): `python scripts/build-mod.py`, then copy
`dist/vantage.wasm` to `app/builtin/vantage.wasm`.

Don't also import Vantage under Settings > Mods: two copies would both draw.
