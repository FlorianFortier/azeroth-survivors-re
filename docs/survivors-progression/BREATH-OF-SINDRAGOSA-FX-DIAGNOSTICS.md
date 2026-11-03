# Breath of Sindragosa — FX diagnostics

This change adds targeted diagnostics to the existing Legion spell-kit effect path. It does not alter damage, cooldown, target selection, the SpellKitFx message shape, or the multiplayer protocol.

When SpellID 152279 is cast, the logs identify the Survivors cast, receipt of SpellKitFx::Begin, each M2 request and canonical mpq:// URL, a one-time warning while the model has not built display parts, a success record when the model is attached (including requested/resolved attachment tags, geometry/effects counts, animation clips and sequence span), and a warning if the one-shot times out before attachment.

The pending warning is emitted once per model path, not once per frame.

Run with:

```powershell
$env:RUST_LOG="info,benilla_app::survivors::units=debug,benilla_app::entities::spell_fx=debug"
```

The next local cast should distinguish a missing cast event, a model-load/build stall, an empty renderable model, or an attachment/lifecycle problem.

The repository authoring environment cannot run the Windows build or access the user's CASC installation. No gameplay, profile, simulation or protocol behavior is changed.
