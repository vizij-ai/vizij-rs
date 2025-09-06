# @vizij/animation-wasm (published from vizij-rs)

Wrapper around the wasm-pack output for the Vizij animation controller.

## Build (local dev)
```bash
# from vizij-rs/
wasm-pack build crates/animation/vizij-animation-wasm --target web --out-dir npm/@vizij/animation-wasm/pkg --release
cd npm/@vizij/animation-wasm
npm i && npm run build
```

## Link to vizij-web (local dev)
```bash
cd npm/@vizij/animation-wasm
npm link

# in vizij-web/
npm link @vizij/animation-wasm
```

## Use
```ts
import init, { Animation } from "@vizij/animation-wasm";
await init();
const anim = new Animation({ frequency_hz: 1.0 });
const out = anim.update(0.016, {});
```
