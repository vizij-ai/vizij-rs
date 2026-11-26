---
"@vizij/runtime": major
---

A drag on a face's control has a lifecycle a driver can build an edit on, in the shape `@semio/scene` gives robots' controls. Each drag sends one `start`, a `move` for each frame its pose or scale changes, and exactly one closing event: `end` when the pointer lets go, or `cancel` when the drag is abandoned — by `vizijs.cancelDrag()`, or by its control being removed, replaced, hidden or made not `interactive`, or its face unloaded or reloaded, while the button is down. A `cancel` carries no pose or scale: the driver drops what the drag previewed.

- `ControlEvent` gains `phase: "cancel"`, `gesture` (the same on every event of one drag, different for every other drag) and `modifiers` (`{ alt, shift, meta, ctrl }`, the `Modifiers` type).
- `vizijs.cancelDrag()` abandons the drag in hand, for what the faces cannot see themselves: Escape, the window losing focus, a `pointercancel`, the page hidden.
- `vizijs.setModifiers(modifiers)` says which keys are held, for every `control` event from the next frame on; the canvas sees no keys while it lacks focus, so the page passes them from its own pointer and key events.
- A per-axis rotation write (`setValue(instance, element, value, "rotation.r")`) keeps the angles last written to its node, a pitch past ±90° included, while the node holds the rotation they made, and starts from where anything else left the node otherwise.

Breaking for a page that handles `control` events:

- A drag can close with `cancel` instead of `end`, whether or not the page calls `cancelDrag()`: removing, replacing or hiding a control mid-drag, or unloading its face, sends one. A page that keeps state per drag until `end` has to drop it on `cancel` too, and code that handles every `phase` has to handle `"cancel"`.
- `gesture` and `modifiers` are required on `ControlEvent`, so an event built by hand, a test's stand-in for example, has to carry them.
