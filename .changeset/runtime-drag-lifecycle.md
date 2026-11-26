---
"@vizij/runtime": minor
---

A drag on a face's control has a lifecycle a driver can build an edit on, in the shape `@semio/scene` gives robots' controls. Each drag sends one `start`, a `move` for each frame its pose or scale changes, and exactly one closing event: `end` when the pointer lets go, or `cancel` when the drag is abandoned — by `vizijs.cancelDrag()`, or by its control being removed, replaced, hidden or made not `interactive`, or its face unloaded or reloaded, while the button is down. A `cancel` carries no pose or scale: the driver drops what the drag previewed.

- `ControlEvent` gains `phase: "cancel"`, `gesture` (the same on every event of one drag, different for every other drag) and `modifiers` (`{ alt, shift, meta, ctrl }`, the `Modifiers` type).
- `vizijs.cancelDrag()` abandons the drag in hand, for what the faces cannot see themselves: Escape, the window losing focus, a `pointercancel`, the page hidden.
- `vizijs.setModifiers(modifiers)` says which keys are held, for every `control` event from the next frame on; the canvas sees no keys while it lacks focus, so the page passes them from its own pointer and key events.
- A per-axis rotation write (`setValue(instance, element, value, "rotation.r")`) keeps the angles last written to its node, a pitch past ±90° included, while the node holds the rotation they made, and starts from where anything else left the node otherwise.
